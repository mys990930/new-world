use std::env;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::Instant;

use image::RgbaImage;
use new_world::renderer::OffscreenRenderOutput;
use new_world::world::CHUNK_EDGE_I32;
use new_world::world::WorldMeta;
use new_world::world::generation::{
    BoundaryCache, BoundaryConfig, DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS,
    GraphHydrologyGraph, GraphMacroMap, GraphRegionArea, GraphRegionCoord, HeightfieldColumn,
    HeightfieldConfig, HeightfieldTerrainKind, HeightfieldTile, MacroFieldTile,
    MacroFieldTileConfig, MacroMapConfig, VoronoiGraphConfig, VoronoiGraphPatch,
    VoronoiGraphPatchRequest, generate_heightfield_tile, generate_macro_field_tile,
    generate_macro_map, generate_noisy_boundaries, generate_voronoi_graph_patch,
    graph_region_for_world_block, solve_hydrology,
};

const DEFAULT_IMAGE_WIDTH: u32 = 1280;
const DEFAULT_IMAGE_HEIGHT: u32 = 720;
const DEFAULT_WORLD_SPAN_BLOCKS: i32 = 8192;
const DEFAULT_COLUMNS_X: u32 = 192;
const DEFAULT_VERTICAL_SCALE: f32 = 1.0;
const WATER_ALPHA: f32 = 0.72;
const ISO_TILE_HEIGHT_RATIO: f32 = 0.50;
const ISO_TARGET_RELIEF_FRACTION: f32 = 0.28;
const ISO_MIN_RELIEF_FRACTION: f32 = 0.20;
const ISO_MAX_RELIEF_FRACTION: f32 = 0.35;
const MACRO_FIELD_TILE_EDGE_BLOCKS: i32 = DEFAULT_GRAPH_REGION_SIZE_BLOCKS;
const PREVIEW_MAJOR_CHUNK_GRID_MULTIPLIER: i32 = 8;
const PREVIEW_MAJOR_CHUNK_GRID_BLOCKS: i32 = CHUNK_EDGE_I32 * PREVIEW_MAJOR_CHUNK_GRID_MULTIPLIER;

#[derive(Debug, Clone)]
struct PreviewConfig {
    seed: u64,
    center_x: i32,
    center_z: i32,
    width: u32,
    height: u32,
    world_span_blocks: i32,
    region_size_blocks: i32,
    site_spacing_blocks: i32,
    land_bias: f32,
    columns_x: u32,
    columns_z: Option<u32>,
    chunk_radius: Option<i32>,
    quarter_turns: u8,
    vertical_scale: f32,
    output: Option<PathBuf>,
}

impl PreviewConfig {
    fn validate(self) -> Result<Self, Box<dyn Error>> {
        if self.width == 0 || self.height == 0 {
            return Err(cli_error("width and height must be positive"));
        }
        if self.world_span_blocks <= 0 {
            return Err(cli_error("world-span-blocks must be positive"));
        }
        if self.region_size_blocks <= 0 || self.site_spacing_blocks <= 0 {
            return Err(cli_error("region and site spacing must be positive"));
        }
        if self.columns_x == 0 || self.columns_z == Some(0) {
            return Err(cli_error("columns-x and columns-z must be positive"));
        }
        if self.chunk_radius.is_some_and(|radius| radius < 0) {
            return Err(cli_error("chunk-radius must be zero or positive"));
        }
        if !self.vertical_scale.is_finite() || self.vertical_scale <= 0.0 {
            return Err(cli_error("vertical-scale must be a positive finite number"));
        }
        Ok(self)
    }

    fn columns_z(&self) -> u32 {
        self.columns_z.unwrap_or_else(|| match self.chunk_radius {
            Some(_) => self.columns_x,
            None => {
                ((self.columns_x as f32 * self.height as f32 / self.width as f32)
                    .round()
                    .max(1.0)) as u32
            }
        })
    }

    fn window(&self) -> PreviewWindow {
        let (center_x, center_z, span_x, span_z) = if let Some(radius) = self.chunk_radius {
            let center_chunk_x = self.center_x.div_euclid(CHUNK_EDGE_I32);
            let center_chunk_z = self.center_z.div_euclid(CHUNK_EDGE_I32);
            let min_chunk_x = center_chunk_x - radius;
            let max_chunk_x = center_chunk_x + radius;
            let min_chunk_z = center_chunk_z - radius;
            let max_chunk_z = center_chunk_z + radius;
            let min_x = min_chunk_x * CHUNK_EDGE_I32;
            let max_x = (max_chunk_x + 1) * CHUNK_EDGE_I32;
            let min_z = min_chunk_z * CHUNK_EDGE_I32;
            let max_z = (max_chunk_z + 1) * CHUNK_EDGE_I32;
            (
                (min_x + max_x) as f32 * 0.5,
                (min_z + max_z) as f32 * 0.5,
                (max_x - min_x) as f32,
                (max_z - min_z) as f32,
            )
        } else {
            let span_x = self.world_span_blocks as f32;
            let span_z = span_x * self.columns_z() as f32 / self.columns_x as f32;
            (self.center_x as f32, self.center_z as f32, span_x, span_z)
        };
        PreviewWindow {
            center_x,
            center_z,
            columns_x: self.columns_x,
            columns_z: self.columns_z(),
            world_span_x: span_x,
            world_span_z: span_z,
        }
    }

    fn output_path(&self) -> PathBuf {
        self.output.clone().unwrap_or_else(|| {
            PathBuf::from(format!(
                "target/heightfield-preview/s{}_x{}_z{}.png",
                self.seed, self.center_x, self.center_z
            ))
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct PreviewWindow {
    center_x: f32,
    center_z: f32,
    columns_x: u32,
    columns_z: u32,
    world_span_x: f32,
    world_span_z: f32,
}

impl PreviewWindow {
    fn min_x(self) -> f32 {
        self.center_x - self.world_span_x * 0.5
    }

    fn max_x(self) -> f32 {
        self.center_x + self.world_span_x * 0.5
    }

    fn min_z(self) -> f32 {
        self.center_z - self.world_span_z * 0.5
    }

    fn max_z(self) -> f32 {
        self.center_z + self.world_span_z * 0.5
    }

    fn sample_spacing(self) -> f32 {
        self.world_span_x / self.columns_x as f32
    }

    fn graph_area(self, region_size_blocks: i32) -> Result<GraphRegionArea, Box<dyn Error>> {
        let min = graph_region_for_world_block(
            self.min_x().floor() as i32,
            self.min_z().floor() as i32,
            region_size_blocks,
        );
        let max = graph_region_for_world_block(
            self.max_x().ceil() as i32,
            self.max_z().ceil() as i32,
            region_size_blocks,
        );
        GraphRegionArea::new(min, max).ok_or_else(|| cli_error("invalid heightfield preview area"))
    }
}

#[derive(Debug, Clone)]
struct PreviewHeader {
    seed: u64,
    generator_version: u32,
    center_x: i32,
    center_z: i32,
    width: u32,
    height: u32,
    world_span_blocks: i32,
    world_min_x: f32,
    world_max_x: f32,
    world_min_z: f32,
    world_max_z: f32,
    columns_x: u32,
    columns_z: u32,
    sample_spacing_blocks: f32,
    chunk_edge_blocks: i32,
    chunk_min_x: i32,
    chunk_max_x: i32,
    chunk_min_z: i32,
    chunk_max_z: i32,
    chunk_radius_x: i32,
    chunk_radius_z: i32,
    requested_chunk_radius: Option<i32>,
    major_grid_edge_blocks: i32,
    macro_tile_edge_blocks: i32,
    vertical_scale: f32,
    graph_site_count: usize,
    macro_sample_count: usize,
    column_count: usize,
    min_surface: f32,
    avg_surface: f32,
    max_surface: f32,
    max_raw_neighbor_delta: f32,
    max_constrained_neighbor_delta: f32,
    max_snapped_neighbor_delta: f32,
    max_visible_neighbor_delta: f32,
    vertical_px_per_block: f32,
    projected_height_span_px: f32,
    water_columns: usize,
    ocean_columns: usize,
    lake_columns: usize,
    river_columns: usize,
    dry_basin_columns: usize,
    ridge_columns: usize,
    build_ms: u128,
    macro_field_ms: u128,
    heightfield_ms: u128,
    mesh_ms: u128,
    render_ms: u128,
    total_ms: u128,
}

impl PreviewHeader {
    fn to_metadata_text(&self) -> String {
        [
            "stage=heightfield".to_string(),
            format!("seed={}", self.seed),
            format!("generator_version={}", self.generator_version),
            format!("center={},{}", self.center_x, self.center_z),
            format!("image={}x{}", self.width, self.height),
            format!("world_span_blocks={}", self.world_span_blocks),
            format!(
                "world_footprint_blocks=x:{:.1}..{:.1},z:{:.1}..{:.1}",
                self.world_min_x, self.world_max_x, self.world_min_z, self.world_max_z
            ),
            format!("columns={}x{}", self.columns_x, self.columns_z),
            format!("sample_spacing_blocks={:.3}", self.sample_spacing_blocks),
            format!("chunk_edge_blocks={}", self.chunk_edge_blocks),
            format!("major_chunk_grid_blocks={}", self.major_grid_edge_blocks),
            format!(
                "chunk_range_xz={}..{},{}..{}",
                self.chunk_min_x, self.chunk_max_x, self.chunk_min_z, self.chunk_max_z
            ),
            format!(
                "chunk_radius_xz={},{}",
                self.chunk_radius_x, self.chunk_radius_z
            ),
            format!(
                "requested_chunk_radius={}",
                self.requested_chunk_radius
                    .map_or_else(|| "derived".to_string(), |radius| radius.to_string())
            ),
            format!(
                "macro_field_tile_edge_blocks={}",
                self.macro_tile_edge_blocks
            ),
            format!("vertical_scale={:.3}", self.vertical_scale),
            "view=cpu_isometric_columns".to_string(),
            "projection=screen_x_(x-z)*tile_w/2_screen_y_(x+z)*tile_h/2-y*vertical_px".to_string(),
            format!("vertical_px_per_block={:.4}", self.vertical_px_per_block),
            format!(
                "projected_height_span_px={:.3}",
                self.projected_height_span_px
            ),
            format!("graph_sites={}", self.graph_site_count),
            format!("macro_samples={}", self.macro_sample_count),
            format!("heightfield_columns={}", self.column_count),
            format!(
                "surface_min_avg_max_blocks={:.3},{:.3},{:.3}",
                self.min_surface, self.avg_surface, self.max_surface
            ),
            format!(
                "neighbor_delta_raw_constrained_snapped_visible_blocks={:.3},{:.3},{:.3},{:.3}",
                self.max_raw_neighbor_delta,
                self.max_constrained_neighbor_delta,
                self.max_snapped_neighbor_delta,
                self.max_visible_neighbor_delta
            ),
            format!(
                "water_ocean_lake_river_dry_ridge_columns={},{},{},{},{},{}",
                self.water_columns,
                self.ocean_columns,
                self.lake_columns,
                self.river_columns,
                self.dry_basin_columns,
                self.ridge_columns
            ),
            "meso_delta_blocks=0".to_string(),
            "micro_relief_blocks=0".to_string(),
            "height_snap=round_to_integer_block".to_string(),
            "shoreline_policy=land_coast_mask_ramps_from_y0_without_vertical_sea_cliff".to_string(),
            "height_mapping=combined_macro_height_-0.75_to_1.25_maps_-48_to_160_blocks".to_string(),
            format!(
                "timing_ms=build:{} macro_field:{} heightfield:{} projection:{} render:{} total:{}",
                self.build_ms,
                self.macro_field_ms,
                self.heightfield_ms,
                self.mesh_ms,
                self.render_ms,
                self.total_ms
            ),
            "meaning=diagnostic_voxelized_heightfield_columns_not_final_chunkdata".to_string(),
        ]
        .join("\n")
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let total_start = Instant::now();
    let config = parse_args()?.validate()?;
    let output = config.output_path();
    let meta = WorldMeta::new(config.seed);
    let window = config.window();
    let graph_area = window.graph_area(config.region_size_blocks)?;

    let build_start = Instant::now();
    let (patch, macro_map, hydrology, boundary) =
        build_generation_inputs(&meta, &config, graph_area)?;
    let build_ms = build_start.elapsed().as_millis();

    let macro_start = Instant::now();
    let macro_tile = build_macro_field_tile(window, &patch, &macro_map, &hydrology, &boundary);
    let macro_field_ms = macro_start.elapsed().as_millis();

    let heightfield_start = Instant::now();
    let heightfield = generate_heightfield_tile(&macro_tile, HeightfieldConfig::default());
    let heightfield_ms = heightfield_start.elapsed().as_millis();

    let mesh_start = Instant::now();
    let plan = IsoRenderPlan::new(
        &heightfield,
        config.width,
        config.height,
        config.quarter_turns % 4,
        config.vertical_scale,
    )?;
    let mesh_ms = mesh_start.elapsed().as_millis();

    let render_start = Instant::now();
    let (mut image, iso_stats) = render_heightfield_isometric(&heightfield, plan)?;
    let render_ms = render_start.elapsed().as_millis();

    let total_ms = total_start.elapsed().as_millis();
    let chunk_range = chunk_range_for_window(window);
    let center_chunk_x = config.center_x.div_euclid(CHUNK_EDGE_I32);
    let center_chunk_z = config.center_z.div_euclid(CHUNK_EDGE_I32);
    let header = PreviewHeader {
        seed: config.seed,
        generator_version: meta.generator_version,
        center_x: config.center_x,
        center_z: config.center_z,
        width: config.width,
        height: config.height,
        world_span_blocks: window.world_span_x.round() as i32,
        world_min_x: window.min_x(),
        world_max_x: window.max_x(),
        world_min_z: window.min_z(),
        world_max_z: window.max_z(),
        columns_x: window.columns_x,
        columns_z: window.columns_z,
        sample_spacing_blocks: window.sample_spacing(),
        chunk_edge_blocks: CHUNK_EDGE_I32,
        chunk_min_x: chunk_range.0,
        chunk_max_x: chunk_range.1,
        chunk_min_z: chunk_range.2,
        chunk_max_z: chunk_range.3,
        chunk_radius_x: (center_chunk_x - chunk_range.0)
            .abs()
            .max((chunk_range.1 - center_chunk_x).abs()),
        chunk_radius_z: (center_chunk_z - chunk_range.2)
            .abs()
            .max((chunk_range.3 - center_chunk_z).abs()),
        requested_chunk_radius: config.chunk_radius,
        major_grid_edge_blocks: PREVIEW_MAJOR_CHUNK_GRID_BLOCKS,
        macro_tile_edge_blocks: MACRO_FIELD_TILE_EDGE_BLOCKS,
        vertical_scale: config.vertical_scale,
        graph_site_count: patch.sites.len(),
        macro_sample_count: macro_tile.samples.len(),
        column_count: heightfield.stats.column_count,
        min_surface: heightfield.stats.min_surface_height_blocks,
        avg_surface: heightfield.stats.average_surface_height_blocks,
        max_surface: heightfield.stats.max_surface_height_blocks,
        max_raw_neighbor_delta: heightfield.stats.max_raw_neighbor_delta_blocks,
        max_constrained_neighbor_delta: heightfield.stats.max_constrained_neighbor_delta_blocks,
        max_snapped_neighbor_delta: heightfield.stats.max_snapped_neighbor_delta_blocks,
        max_visible_neighbor_delta: max_visible_neighbor_delta(&heightfield),
        vertical_px_per_block: iso_stats.vertical_px_per_block,
        projected_height_span_px: iso_stats.projected_height_span_px,
        water_columns: heightfield.stats.water_column_count,
        ocean_columns: heightfield.stats.ocean_column_count,
        lake_columns: heightfield.stats.lake_column_count,
        river_columns: heightfield.stats.river_hint_column_count,
        dry_basin_columns: heightfield.stats.dry_basin_column_count,
        ridge_columns: heightfield.stats.ridge_column_count,
        build_ms,
        macro_field_ms,
        heightfield_ms,
        mesh_ms,
        render_ms,
        total_ms,
    };
    draw_boundary_overlays(&mut image, &heightfield, plan, window);
    draw_overlay(&mut image, &header);
    write_rgba_png_with_metadata(&image, &output, &header)?;

    println!("heightfield preview: seed {}", config.seed);
    println!(
        "window: center=({}, {}), span={:.0}x{:.0} blocks, columns={}x{}, spacing={:.2} blocks",
        config.center_x,
        config.center_z,
        window.world_span_x,
        window.world_span_z,
        window.columns_x,
        window.columns_z,
        window.sample_spacing()
    );
    if let Some(radius) = config.chunk_radius {
        println!(
            "chunk-radius input: square radius {} around center chunk ({}, {})",
            radius, center_chunk_x, center_chunk_z
        );
    }
    println!(
        "footprint: x {:.1}..{:.1}, z {:.1}..{:.1} blocks",
        window.min_x(),
        window.max_x(),
        window.min_z(),
        window.max_z()
    );
    println!(
        "grid overlay: chunk {} blocks faint, major {} blocks, macro tile {} blocks",
        CHUNK_EDGE_I32, PREVIEW_MAJOR_CHUNK_GRID_BLOCKS, MACRO_FIELD_TILE_EDGE_BLOCKS
    );
    println!(
        "chunk range: cx {}..{}, cz {}..{}, radius {}x{}",
        chunk_range.0,
        chunk_range.1,
        chunk_range.2,
        chunk_range.3,
        header.chunk_radius_x,
        header.chunk_radius_z
    );
    println!(
        "view: cpu isometric columns, quarter turns {}, vertical scale multiplier {:.2}, vertical {:.3} px/block, relief span {:.1}px",
        config.quarter_turns % 4,
        config.vertical_scale,
        iso_stats.vertical_px_per_block,
        iso_stats.projected_height_span_px
    );
    println!(
        "surface height min/avg/max {:.2}/{:.2}/{:.2} blocks",
        heightfield.stats.min_surface_height_blocks,
        heightfield.stats.average_surface_height_blocks,
        heightfield.stats.max_surface_height_blocks
    );
    println!(
        "neighbor delta raw/constrained/snapped/visible max {:.2}/{:.2}/{:.2}/{:.2} blocks",
        heightfield.stats.max_raw_neighbor_delta_blocks,
        heightfield.stats.max_constrained_neighbor_delta_blocks,
        heightfield.stats.max_snapped_neighbor_delta_blocks,
        header.max_visible_neighbor_delta
    );
    println!(
        "columns: total {}, water {}, ocean {}, lake {}, river {}, dry {}, ridge {}",
        heightfield.stats.column_count,
        heightfield.stats.water_column_count,
        heightfield.stats.ocean_column_count,
        heightfield.stats.lake_column_count,
        heightfield.stats.river_hint_column_count,
        heightfield.stats.dry_basin_column_count,
        heightfield.stats.ridge_column_count
    );
    println!(
        "timing: build {} ms, macro field {} ms, heightfield {} ms, projection {} ms, render {} ms, total {} ms",
        build_ms, macro_field_ms, heightfield_ms, mesh_ms, render_ms, total_ms
    );
    println!("output: {}", output.display());
    println!("metadata: new-world-preview-header iTXt chunk");

    Ok(())
}

fn build_generation_inputs(
    meta: &WorldMeta,
    config: &PreviewConfig,
    graph_area: GraphRegionArea,
) -> Result<
    (
        VoronoiGraphPatch,
        GraphMacroMap,
        GraphHydrologyGraph,
        BoundaryCache,
    ),
    Box<dyn Error>,
> {
    let center_region =
        graph_region_for_world_block(config.center_x, config.center_z, config.region_size_blocks);
    let padding_regions = required_padding_regions(center_region, graph_area)?;
    let graph_config = VoronoiGraphConfig {
        seed: meta.seed,
        generator_version: meta.generator_version,
        region_size_blocks: config.region_size_blocks,
        site_spacing_blocks: config.site_spacing_blocks,
        padding_regions,
    };
    let patch = generate_voronoi_graph_patch(VoronoiGraphPatchRequest::new(
        graph_config,
        config.center_x,
        config.center_z,
    ));
    let macro_map = generate_macro_map(
        &patch,
        MacroMapConfig {
            land_bias: config.land_bias,
            ..MacroMapConfig::new(meta.seed, meta.generator_version)
        },
    );
    let hydrology = solve_hydrology(&patch, &macro_map, Default::default());
    let boundary = generate_noisy_boundaries(
        &patch,
        &macro_map,
        BoundaryConfig::new(meta.seed, meta.generator_version),
    );

    Ok((patch, macro_map, hydrology, boundary))
}

fn required_padding_regions(
    center: GraphRegionCoord,
    area: GraphRegionArea,
) -> Result<u32, Box<dyn Error>> {
    let dx = (center.x - area.min.x)
        .abs()
        .max((area.max.x - center.x).abs());
    let dz = (center.z - area.min.z)
        .abs()
        .max((area.max.z - center.z).abs());
    u32::try_from(dx.max(dz).saturating_add(1))
        .map_err(|_| cli_error("heightfield preview padding overflowed"))
}

fn build_macro_field_tile(
    window: PreviewWindow,
    patch: &VoronoiGraphPatch,
    macro_map: &GraphMacroMap,
    hydrology: &GraphHydrologyGraph,
    boundary: &BoundaryCache,
) -> MacroFieldTile {
    let sample_spacing = window.sample_spacing();
    let config = MacroFieldTileConfig::new(
        window.min_x() + sample_spacing * 0.5,
        window.min_z() + sample_spacing * 0.5,
        window.columns_x,
        window.columns_z,
        sample_spacing,
    );

    generate_macro_field_tile(patch, macro_map, hydrology, boundary, config)
}

fn chunk_range_for_window(window: PreviewWindow) -> (i32, i32, i32, i32) {
    let min_x = window.min_x().floor() as i32;
    let max_x = (window.max_x().ceil() as i32).saturating_sub(1);
    let min_z = window.min_z().floor() as i32;
    let max_z = (window.max_z().ceil() as i32).saturating_sub(1);
    (
        min_x.div_euclid(CHUNK_EDGE_I32),
        max_x.div_euclid(CHUNK_EDGE_I32),
        min_z.div_euclid(CHUNK_EDGE_I32),
        max_z.div_euclid(CHUNK_EDGE_I32),
    )
}

#[derive(Debug, Clone, Copy)]
struct IsoRenderPlan {
    width: u32,
    height: u32,
    quarter_turns: u8,
    tile_w_px: f32,
    tile_h_px: f32,
    vertical_px_per_block: f32,
    offset_x_px: f32,
    offset_y_px: f32,
    min_surface_blocks: f32,
    max_surface_blocks: f32,
}

#[derive(Debug, Clone, Copy)]
struct IsoRenderStats {
    vertical_px_per_block: f32,
    projected_height_span_px: f32,
}

#[derive(Debug, Clone, Copy)]
struct Point2 {
    x: f32,
    y: f32,
}

impl IsoRenderPlan {
    fn new(
        tile: &HeightfieldTile,
        width: u32,
        height: u32,
        quarter_turns: u8,
        vertical_scale: f32,
    ) -> Result<Self, Box<dyn Error>> {
        if tile.columns.is_empty() {
            return Err(cli_error("heightfield preview cannot render an empty tile"));
        }
        let min_surface = tile.stats.min_surface_height_blocks;
        let max_surface = tile.stats.max_surface_height_blocks;
        let height_range = (max_surface - min_surface).max(1.0);
        let relief_fraction = (ISO_TARGET_RELIEF_FRACTION * vertical_scale)
            .clamp(ISO_MIN_RELIEF_FRACTION, ISO_MAX_RELIEF_FRACTION);
        let target_relief_px = height as f32 * relief_fraction;
        let footprint_axis_count = (tile.width + tile.height).max(2) as f32;
        let tile_w_by_width = width as f32 * 1.64 / footprint_axis_count;
        let height_budget = (height as f32 * 0.86 - target_relief_px).max(height as f32 * 0.34);
        let tile_w_by_height = height_budget * 4.0 / footprint_axis_count;
        let tile_w_px = tile_w_by_width.min(tile_w_by_height).clamp(2.0, 24.0);
        let tile_h_px = tile_w_px * ISO_TILE_HEIGHT_RATIO;
        let vertical_px_per_block = (target_relief_px / height_range).max(0.05);
        let mut plan = Self {
            width,
            height,
            quarter_turns: quarter_turns % 4,
            tile_w_px,
            tile_h_px,
            vertical_px_per_block,
            offset_x_px: 0.0,
            offset_y_px: 0.0,
            min_surface_blocks: min_surface,
            max_surface_blocks: max_surface,
        };
        let (min_x, max_x, min_y, max_y) = plan.untranslated_bounds(tile);
        plan.offset_x_px = width as f32 * 0.5 - (min_x + max_x) * 0.5;
        plan.offset_y_px = height as f32 * 0.5 - (min_y + max_y) * 0.5;
        Ok(plan)
    }

    fn projected_height_span_px(self) -> f32 {
        (self.max_surface_blocks - self.min_surface_blocks).max(0.0) * self.vertical_px_per_block
    }

    fn project_grid(self, x: f32, z: f32, y_blocks: f32, tile: &HeightfieldTile) -> Point2 {
        let center_x = tile.width as f32 * 0.5;
        let center_z = tile.height as f32 * 0.5;
        let dx = x - center_x;
        let dz = z - center_z;
        let (rx, rz) = match self.quarter_turns {
            0 => (dx, dz),
            1 => (dz, -dx),
            2 => (-dx, -dz),
            3 => (-dz, dx),
            _ => unreachable!(),
        };
        Point2 {
            x: (rx - rz) * self.tile_w_px * 0.5 + self.offset_x_px,
            y: (rx + rz) * self.tile_h_px * 0.5 - y_blocks * self.vertical_px_per_block
                + self.offset_y_px,
        }
    }

    fn untranslated_project_grid(
        self,
        x: f32,
        z: f32,
        y_blocks: f32,
        tile: &HeightfieldTile,
    ) -> Point2 {
        let mut plan = self;
        plan.offset_x_px = 0.0;
        plan.offset_y_px = 0.0;
        plan.project_grid(x, z, y_blocks, tile)
    }

    fn untranslated_bounds(self, tile: &HeightfieldTile) -> (f32, f32, f32, f32) {
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        for x in [0.0, tile.width as f32] {
            for z in [0.0, tile.height as f32] {
                for y in [self.min_surface_blocks, self.max_surface_blocks, 0.0] {
                    let p = self.untranslated_project_grid(x, z, y, tile);
                    min_x = min_x.min(p.x);
                    max_x = max_x.max(p.x);
                    min_y = min_y.min(p.y);
                    max_y = max_y.max(p.y);
                }
            }
        }
        (min_x, max_x, min_y, max_y)
    }
}

fn render_heightfield_isometric(
    tile: &HeightfieldTile,
    plan: IsoRenderPlan,
) -> Result<(OffscreenRenderOutput, IsoRenderStats), Box<dyn Error>> {
    let mut image = RgbaImage::from_pixel(plan.width, plan.height, image::Rgba([12, 15, 18, 255]));
    let width = tile.width as usize;
    let height = tile.height as usize;
    for diagonal in 0..(width + height - 1) {
        for z in 0..height {
            if diagonal < z {
                continue;
            }
            let x = diagonal - z;
            if x >= width {
                continue;
            }
            let index = z * width + x;
            draw_column_iso(&mut image, tile, plan, x, z, tile.columns[index]);
        }
    }
    Ok((
        OffscreenRenderOutput {
            width: plan.width,
            height: plan.height,
            rgba: image.into_raw(),
            draw_call_count: (tile.columns.len() * 3) as u32,
        },
        IsoRenderStats {
            vertical_px_per_block: plan.vertical_px_per_block,
            projected_height_span_px: plan.projected_height_span_px(),
        },
    ))
}

fn draw_column_iso(
    image: &mut RgbaImage,
    tile: &HeightfieldTile,
    plan: IsoRenderPlan,
    x: usize,
    z: usize,
    column: HeightfieldColumn,
) {
    let surface = column.surface_height_blocks;
    let visible_surface = visible_surface_height(column);
    let width = tile.width as usize;
    let height = tile.height as usize;
    let east = if x + 1 < width {
        visible_surface_height(tile.columns[z * width + x + 1])
    } else {
        visible_surface - 12.0
    };
    let south = if z + 1 < height {
        visible_surface_height(tile.columns[(z + 1) * width + x])
    } else {
        visible_surface - 12.0
    };
    let color = terrain_color_rgba(column);
    if visible_surface > east + 0.75 {
        draw_side_face(
            image,
            tile,
            plan,
            [(x + 1) as f32, z as f32, (x + 1) as f32, (z + 1) as f32],
            east,
            visible_surface,
            shade_rgba(color, 0.68),
        );
    }
    if visible_surface > south + 0.75 {
        draw_side_face(
            image,
            tile,
            plan,
            [x as f32, (z + 1) as f32, (x + 1) as f32, (z + 1) as f32],
            south,
            visible_surface,
            shade_rgba(color, 0.56),
        );
    }
    draw_top_face(image, tile, plan, x, z, surface, shade_rgba(color, 1.05));

    if let Some(water) = column.water_level_blocks {
        if water > surface {
            draw_top_face(
                image,
                tile,
                plan,
                x,
                z,
                water + 0.10,
                water_color_rgba(column),
            );
        }
    }
}

fn visible_surface_height(column: HeightfieldColumn) -> f32 {
    column
        .water_level_blocks
        .filter(|water| *water > column.surface_height_blocks)
        .unwrap_or(column.surface_height_blocks)
}

fn max_visible_neighbor_delta(tile: &HeightfieldTile) -> f32 {
    let width = tile.width as usize;
    let height = tile.height as usize;
    let mut max_delta = 0.0f32;
    for z in 0..height {
        for x in 0..width {
            let index = z * width + x;
            let here = visible_surface_height(tile.columns[index]);
            if x + 1 < width {
                max_delta =
                    max_delta.max((here - visible_surface_height(tile.columns[index + 1])).abs());
            }
            if z + 1 < height {
                max_delta = max_delta
                    .max((here - visible_surface_height(tile.columns[index + width])).abs());
            }
        }
    }
    max_delta
}

fn draw_top_face(
    image: &mut RgbaImage,
    tile: &HeightfieldTile,
    plan: IsoRenderPlan,
    x: usize,
    z: usize,
    y: f32,
    color: [u8; 4],
) {
    let polygon = [
        plan.project_grid(x as f32, z as f32, y, tile),
        plan.project_grid((x + 1) as f32, z as f32, y, tile),
        plan.project_grid((x + 1) as f32, (z + 1) as f32, y, tile),
        plan.project_grid(x as f32, (z + 1) as f32, y, tile),
    ];
    fill_convex_polygon(image, &polygon, color);
}

fn draw_side_face(
    image: &mut RgbaImage,
    tile: &HeightfieldTile,
    plan: IsoRenderPlan,
    edge: [f32; 4],
    lower_y: f32,
    upper_y: f32,
    color: [u8; 4],
) {
    let lower_y = lower_y.max(upper_y - 96.0);
    let polygon = [
        plan.project_grid(edge[0], edge[1], upper_y, tile),
        plan.project_grid(edge[2], edge[3], upper_y, tile),
        plan.project_grid(edge[2], edge[3], lower_y, tile),
        plan.project_grid(edge[0], edge[1], lower_y, tile),
    ];
    fill_convex_polygon(image, &polygon, color);
}

fn draw_boundary_overlays(
    image: &mut OffscreenRenderOutput,
    tile: &HeightfieldTile,
    plan: IsoRenderPlan,
    window: PreviewWindow,
) {
    let Some(mut rgba) =
        RgbaImage::from_raw(image.width, image.height, std::mem::take(&mut image.rgba))
    else {
        return;
    };
    draw_world_grid_overlay(
        &mut rgba,
        tile,
        plan,
        window,
        CHUNK_EDGE_I32,
        [24, 32, 39, 30],
        6,
    );
    draw_world_grid_overlay(
        &mut rgba,
        tile,
        plan,
        window,
        PREVIEW_MAJOR_CHUNK_GRID_BLOCKS,
        [84, 112, 126, 82],
        1,
    );
    draw_world_grid_overlay(
        &mut rgba,
        tile,
        plan,
        window,
        MACRO_FIELD_TILE_EDGE_BLOCKS,
        [226, 236, 246, 92],
        1,
    );
    image.rgba = rgba.into_raw();
}

fn draw_world_grid_overlay(
    image: &mut RgbaImage,
    tile: &HeightfieldTile,
    plan: IsoRenderPlan,
    window: PreviewWindow,
    spacing_blocks: i32,
    color: [u8; 4],
    dash_period: i32,
) {
    if spacing_blocks <= 0 {
        return;
    }
    let min_x = window.min_x();
    let min_z = window.min_z();
    let sample_spacing = window.sample_spacing();
    let first_x = (window.min_x().floor() as i32).div_euclid(spacing_blocks) * spacing_blocks;
    let last_x =
        (window.max_x().ceil() as i32).div_euclid(spacing_blocks) * spacing_blocks + spacing_blocks;
    let first_z = (window.min_z().floor() as i32).div_euclid(spacing_blocks) * spacing_blocks;
    let last_z =
        (window.max_z().ceil() as i32).div_euclid(spacing_blocks) * spacing_blocks + spacing_blocks;
    let overlay_y = tile.stats.max_surface_height_blocks + 1.0;

    let mut x = first_x;
    while x <= last_x {
        let gx = (x as f32 - min_x) / sample_spacing;
        if gx >= -1.0 && gx <= tile.width as f32 + 1.0 {
            draw_projected_line(
                image,
                plan.project_grid(gx, 0.0, overlay_y, tile),
                plan.project_grid(gx, tile.height as f32, overlay_y, tile),
                color,
                dash_period,
            );
        }
        x += spacing_blocks;
    }

    let mut z = first_z;
    while z <= last_z {
        let gz = (z as f32 - min_z) / sample_spacing;
        if gz >= -1.0 && gz <= tile.height as f32 + 1.0 {
            draw_projected_line(
                image,
                plan.project_grid(0.0, gz, overlay_y, tile),
                plan.project_grid(tile.width as f32, gz, overlay_y, tile),
                color,
                dash_period,
            );
        }
        z += spacing_blocks;
    }
}

fn draw_projected_line(
    image: &mut RgbaImage,
    start: Point2,
    end: Point2,
    color: [u8; 4],
    dash_period: i32,
) {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let steps = dx.abs().max(dy.abs()).ceil().max(1.0) as i32;
    for step in 0..=steps {
        if dash_period > 1 && (step / dash_period) % 2 != 0 {
            continue;
        }
        let t = step as f32 / steps as f32;
        let x = (start.x + dx * t).round() as i32;
        let y = (start.y + dy * t).round() as i32;
        if x < 0 || y < 0 || x >= image.width() as i32 || y >= image.height() as i32 {
            continue;
        }
        blend_rgba(image, x as u32, y as u32, color, color[3] as f32 / 255.0);
    }
}

fn fill_convex_polygon(image: &mut RgbaImage, points: &[Point2], color: [u8; 4]) {
    if points.len() < 3 {
        return;
    }
    let min_y = points
        .iter()
        .map(|p| p.y)
        .fold(f32::MAX, f32::min)
        .floor()
        .max(0.0) as i32;
    let max_y = points
        .iter()
        .map(|p| p.y)
        .fold(f32::MIN, f32::max)
        .ceil()
        .min(image.height() as f32 - 1.0) as i32;
    for y in min_y..=max_y {
        let scan_y = y as f32 + 0.5;
        let mut xs = Vec::with_capacity(points.len());
        for i in 0..points.len() {
            let a = points[i];
            let b = points[(i + 1) % points.len()];
            if (a.y <= scan_y && b.y > scan_y) || (b.y <= scan_y && a.y > scan_y) {
                let t = (scan_y - a.y) / (b.y - a.y);
                xs.push(a.x + (b.x - a.x) * t);
            }
        }
        if xs.len() < 2 {
            continue;
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        let start = xs[0].floor().max(0.0) as i32;
        let end = xs[xs.len() - 1].ceil().min(image.width() as f32 - 1.0) as i32;
        for x in start..=end {
            blend_rgba(image, x as u32, y as u32, color, color[3] as f32 / 255.0);
        }
    }
}

fn terrain_color_rgba(column: HeightfieldColumn) -> [u8; 4] {
    f32_color_to_rgba(terrain_color_raw(column))
}

fn water_color_rgba(column: HeightfieldColumn) -> [u8; 4] {
    f32_color_to_rgba(water_color_raw(column))
}

fn f32_color_to_rgba(color: [f32; 4]) -> [u8; 4] {
    [
        (color[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[2].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

fn shade_rgba(color: [u8; 4], amount: f32) -> [u8; 4] {
    [
        (color[0] as f32 * amount).round().clamp(0.0, 255.0) as u8,
        (color[1] as f32 * amount).round().clamp(0.0, 255.0) as u8,
        (color[2] as f32 * amount).round().clamp(0.0, 255.0) as u8,
        color[3],
    ]
}

fn terrain_color_raw(column: HeightfieldColumn) -> [f32; 4] {
    let t = ((column.combined_macro_height + 0.75) / 2.0).clamp(0.0, 1.0);
    let mut color = match column.terrain_kind {
        HeightfieldTerrainKind::Ocean => rgb8([45, 78, 102]),
        HeightfieldTerrainKind::Lake => rgb8([55, 100, 124]),
        HeightfieldTerrainKind::River => rgb8([63, 109, 122]),
        HeightfieldTerrainKind::DryBasin => rgb8([118, 111, 119]),
        HeightfieldTerrainKind::Ridge => rgb8([190, 190, 181]),
        HeightfieldTerrainKind::Coast => rgb8([138, 148, 118]),
        HeightfieldTerrainKind::Land => combined_terrain_ramp(t),
    };
    let altitude = (column.surface_height_blocks / 160.0).clamp(-0.2, 0.6);
    for channel in color.iter_mut().take(3) {
        *channel = (*channel * (0.86 + altitude * 0.20)).clamp(0.0, 1.0);
    }
    color
}

fn water_color_raw(column: HeightfieldColumn) -> [f32; 4] {
    let base = if matches!(column.terrain_kind, HeightfieldTerrainKind::Lake) {
        [54, 118, 150]
    } else {
        [46, 92, 130]
    };
    let mut color = rgb8(base);
    color[3] = WATER_ALPHA;
    color
}

fn combined_terrain_ramp(value: f32) -> [f32; 4] {
    let c = gradient_color(
        value,
        &[
            (0.00, [45, 78, 102]),
            (0.28, [67, 111, 123]),
            (0.38, [101, 130, 117]),
            (0.52, [112, 140, 102]),
            (0.68, [143, 145, 116]),
            (0.84, [168, 166, 151]),
            (1.00, [226, 228, 218]),
        ],
    );
    rgb8(c)
}

fn gradient_color(value: f32, stops: &[(f32, [u8; 3])]) -> [u8; 3] {
    let value = value.clamp(0.0, 1.0);
    for pair in stops.windows(2) {
        let (left_t, left_color) = pair[0];
        let (right_t, right_color) = pair[1];
        if value <= right_t {
            let local = if (right_t - left_t).abs() <= f32::EPSILON {
                0.0
            } else {
                (value - left_t) / (right_t - left_t)
            };
            return [
                lerp_channel(left_color[0], right_color[0], local),
                lerp_channel(left_color[1], right_color[1], local),
                lerp_channel(left_color[2], right_color[2], local),
            ];
        }
    }
    stops.last().map_or([0, 0, 0], |(_, color)| *color)
}

fn lerp_channel(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8
}

fn rgb8(color: [u8; 3]) -> [f32; 4] {
    [
        color[0] as f32 / 255.0,
        color[1] as f32 / 255.0,
        color[2] as f32 / 255.0,
        1.0,
    ]
}

fn draw_overlay(image: &mut OffscreenRenderOutput, header: &PreviewHeader) {
    let Some(mut rgba) =
        RgbaImage::from_raw(image.width, image.height, std::mem::take(&mut image.rgba))
    else {
        return;
    };
    let layout = OverlayLayout::new(image.width, image.height);
    let x = layout.margin;
    let y = layout.margin;
    let text_x = x + 4 * layout.scale;
    let mut text_y = y + 4 * layout.scale;
    draw_panel(&mut rgba, x, y, layout.panel_width, layout.panel_height);
    draw_text(
        &mut rgba,
        text_x,
        text_y,
        "ISO HEIGHT",
        [230, 235, 226, 255],
        layout.scale,
    );
    text_y += layout.line_step;
    draw_text(
        &mut rgba,
        text_x,
        text_y,
        &format!("CX {} CZ {}", header.center_x, header.center_z),
        [204, 214, 203, 255],
        layout.scale,
    );
    text_y += layout.line_step;
    draw_text(
        &mut rgba,
        text_x,
        text_y,
        &format!(
            "COL {}X{} STEP {:.0}",
            header.columns_x, header.columns_z, header.sample_spacing_blocks
        ),
        [204, 214, 203, 255],
        layout.scale,
    );
    text_y += layout.line_step;
    draw_text(
        &mut rgba,
        text_x,
        text_y,
        &format!(
            "H {:.0}/{:.0}/{:.0}",
            header.min_surface, header.avg_surface, header.max_surface
        ),
        [204, 214, 203, 255],
        layout.scale,
    );
    text_y += layout.line_step;
    draw_text(
        &mut rgba,
        text_x,
        text_y,
        &format!(
            "CH {}..{} {}..{}",
            header.chunk_min_x, header.chunk_max_x, header.chunk_min_z, header.chunk_max_z
        ),
        [204, 214, 203, 255],
        layout.scale,
    );
    text_y += layout.line_step;
    draw_text(
        &mut rgba,
        text_x,
        text_y,
        &format!(
            "RAD {} BLK {}",
            header.chunk_radius_x.max(header.chunk_radius_z),
            header.world_span_blocks
        ),
        [204, 214, 203, 255],
        layout.scale,
    );
    text_y += layout.line_step;
    draw_legend_keys(&mut rgba, text_x, text_y, layout.scale);
    text_y += layout.line_step;
    draw_grid_legend_keys(&mut rgba, header, text_x, text_y, layout.scale);
    draw_scale_bar(&mut rgba, header, layout.scale);
    image.rgba = rgba.into_raw();
}

#[derive(Debug, Clone, Copy)]
struct OverlayLayout {
    scale: u32,
    margin: u32,
    panel_width: u32,
    panel_height: u32,
    line_step: u32,
}

impl OverlayLayout {
    fn new(width: u32, height: u32) -> Self {
        let min_axis = width.min(height).max(1);
        let scale = ((min_axis as f32 / 360.0).round() as u32).clamp(2, 6);
        let panel_width = ((width as f32 * 0.30).round() as u32)
            .max(136 * scale)
            .min((width as f32 * 0.45).round() as u32);
        let panel_height = ((height as f32 * 0.20).round() as u32)
            .max(58 * scale)
            .min((height as f32 * 0.34).round() as u32);
        Self {
            scale,
            margin: (4 * scale).max(8),
            panel_width,
            panel_height,
            line_step: 8 * scale,
        }
    }
}

fn draw_panel(image: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32) {
    let max_x = (x + w).min(image.width());
    let max_y = (y + h).min(image.height());
    for py in y..max_y {
        for px in x..max_x {
            blend_rgba(image, px, py, [7, 10, 13, 210], 0.72);
        }
    }
}

fn draw_legend_keys(image: &mut RgbaImage, x: u32, y: u32, scale: u32) {
    let keys = [
        ("WTR", [58, 120, 154, 255]),
        ("LOW", [101, 130, 117, 255]),
        ("HI", [190, 190, 181, 255]),
        ("DRY", [118, 111, 119, 255]),
    ];
    let mut cursor = x;
    for (label, color) in keys {
        let swatch = 5 * scale;
        for sy in 0..swatch {
            for sx in 0..swatch {
                set_rgba(image, cursor + sx, y + sy, color);
            }
        }
        draw_text(
            image,
            cursor + swatch + 2 * scale,
            y,
            label,
            [218, 224, 212, 255],
            scale,
        );
        cursor += swatch + (label.len() as u32 * 4 + 6) * scale;
    }
}

fn draw_grid_legend_keys(
    image: &mut RgbaImage,
    header: &PreviewHeader,
    x: u32,
    y: u32,
    scale: u32,
) {
    let keys = [
        (format!("C{}B", header.chunk_edge_blocks), [24, 32, 39, 255]),
        (
            format!("M{}B", header.major_grid_edge_blocks),
            [84, 112, 126, 255],
        ),
        (
            format!("T{}B", header.macro_tile_edge_blocks),
            [226, 236, 246, 255],
        ),
    ];
    let mut cursor = x;
    for (label, color) in keys {
        let swatch = 5 * scale;
        for sy in 0..swatch {
            for sx in 0..swatch {
                set_rgba(image, cursor + sx, y + sy, color);
            }
        }
        draw_text(
            image,
            cursor + swatch + 2 * scale,
            y,
            &label,
            [218, 224, 212, 255],
            scale,
        );
        cursor += swatch + (label.len() as u32 * 4 + 8) * scale;
    }
}

fn draw_scale_bar(image: &mut RgbaImage, header: &PreviewHeader, scale: u32) {
    let scale_blocks = nice_scale_blocks(header.world_span_blocks);
    let px_len = ((scale_blocks as f32 / header.world_span_blocks as f32) * header.width as f32)
        .round()
        .clamp(24.0, header.width as f32 * 0.32) as u32;
    let x: u32 = 16;
    let y = header.height.saturating_sub(24 * scale);
    draw_panel(
        image,
        x.saturating_sub(4),
        y.saturating_sub(6),
        px_len + 70 * scale,
        18 * scale,
    );
    for dx in 0..=px_len {
        for sy in 0..(2 * scale).max(1) {
            set_rgba(image, x + dx, y + sy, [226, 232, 220, 255]);
        }
    }
    for dy in 0..(7 * scale) {
        set_rgba(
            image,
            x,
            y.saturating_sub(2 * scale) + dy,
            [226, 232, 220, 255],
        );
        set_rgba(
            image,
            x + px_len,
            y.saturating_sub(2 * scale) + dy,
            [226, 232, 220, 255],
        );
    }
    draw_text(
        image,
        x + px_len + 6 * scale,
        y.saturating_sub(4 * scale),
        &format!("{} BLK", scale_blocks),
        [226, 232, 220, 255],
        scale,
    );
}

fn nice_scale_blocks(world_span_blocks: i32) -> i32 {
    if world_span_blocks >= 16_384 {
        4096
    } else if world_span_blocks >= 8192 {
        2048
    } else if world_span_blocks >= 4096 {
        1024
    } else {
        512
    }
}

fn draw_text(image: &mut RgbaImage, x: u32, y: u32, text: &str, color: [u8; 4], scale: u32) {
    let mut cursor = x;
    for ch in text.chars() {
        draw_char(image, cursor, y, ch, color, scale);
        cursor = cursor.saturating_add(4 * scale);
    }
}

fn draw_char(image: &mut RgbaImage, x: u32, y: u32, ch: char, color: [u8; 4], scale: u32) {
    let glyph = glyph_3x5(ch);
    for (row, bits) in glyph.iter().enumerate() {
        for col in 0..3 {
            if bits & (1 << (2 - col)) == 0 {
                continue;
            }
            for sy in 0..scale {
                for sx in 0..scale {
                    set_rgba(
                        image,
                        x + col * scale + sx,
                        y + row as u32 * scale + sy,
                        color,
                    );
                }
            }
        }
    }
}

fn glyph_3x5(ch: char) -> [u8; 5] {
    match ch.to_ascii_uppercase() {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b111, 0b001, 0b001, 0b101, 0b010],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b101, 0b111, 0b111, 0b111, 0b101],
        'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'Q' => [0b010, 0b101, 0b101, 0b111, 0b011],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b110, 0b001, 0b010, 0b100, 0b111],
        '3' => [0b110, 0b001, 0b010, 0b001, 0b110],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b110, 0b001, 0b110],
        '6' => [0b011, 0b100, 0b110, 0b101, 0b010],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b010, 0b101, 0b010, 0b101, 0b010],
        '9' => [0b010, 0b101, 0b011, 0b001, 0b110],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        ' ' => [0, 0, 0, 0, 0],
        _ => [0b111, 0b001, 0b010, 0b000, 0b010],
    }
}

fn set_rgba(image: &mut RgbaImage, x: u32, y: u32, color: [u8; 4]) {
    if x < image.width() && y < image.height() {
        image.put_pixel(x, y, image::Rgba(color));
    }
}

fn blend_rgba(image: &mut RgbaImage, x: u32, y: u32, color: [u8; 4], amount: f32) {
    if x >= image.width() || y >= image.height() {
        return;
    }
    let base = image.get_pixel(x, y).0;
    let amount = amount.clamp(0.0, 1.0);
    let blended = [
        ((base[0] as f32 * (1.0 - amount)) + color[0] as f32 * amount) as u8,
        ((base[1] as f32 * (1.0 - amount)) + color[1] as f32 * amount) as u8,
        ((base[2] as f32 * (1.0 - amount)) + color[2] as f32 * amount) as u8,
        255,
    ];
    image.put_pixel(x, y, image::Rgba(blended));
}

fn write_rgba_png_with_metadata(
    image: &OffscreenRenderOutput,
    output: &Path,
    header: &PreviewHeader,
) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(output)?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.add_itxt_chunk(
        "new-world-preview-header".to_string(),
        header.to_metadata_text(),
    )?;
    encoder.add_text_chunk(
        "Software".to_string(),
        "new-world heightfield_preview".to_string(),
    )?;
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&image.rgba)?;
    Ok(())
}

fn parse_args() -> Result<PreviewConfig, Box<dyn Error>> {
    parse_args_from(env::args().skip(1))
}

fn parse_args_from<I, S>(args: I) -> Result<PreviewConfig, Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    if args.len() < 3 {
        return Err(cli_error(usage()));
    }
    let seed = parse_required::<u64>(&mut args, "seed")?;
    let center_x = parse_required::<i32>(&mut args, "center-x")?;
    let center_z = parse_required::<i32>(&mut args, "center-z")?;
    let mut config = PreviewConfig {
        seed,
        center_x,
        center_z,
        width: DEFAULT_IMAGE_WIDTH,
        height: DEFAULT_IMAGE_HEIGHT,
        world_span_blocks: DEFAULT_WORLD_SPAN_BLOCKS,
        region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
        site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
        land_bias: MacroMapConfig::new(seed, WorldMeta::new(seed).generator_version).land_bias,
        columns_x: DEFAULT_COLUMNS_X,
        columns_z: None,
        chunk_radius: None,
        quarter_turns: 0,
        vertical_scale: DEFAULT_VERTICAL_SCALE,
        output: None,
    };

    while !args.is_empty() {
        let flag = args.remove(0);
        match flag.as_str() {
            "--width" => config.width = parse_required::<u32>(&mut args, "width")?,
            "--height" => config.height = parse_required::<u32>(&mut args, "height")?,
            "--world-span-blocks" => {
                config.world_span_blocks = parse_required::<i32>(&mut args, "world-span-blocks")?
            }
            "--region-size-blocks" => {
                config.region_size_blocks = parse_required::<i32>(&mut args, "region-size-blocks")?
            }
            "--site-spacing-blocks" => {
                config.site_spacing_blocks =
                    parse_required::<i32>(&mut args, "site-spacing-blocks")?
            }
            "--land-bias" => config.land_bias = parse_required::<f32>(&mut args, "land-bias")?,
            "--columns-x" => config.columns_x = parse_required::<u32>(&mut args, "columns-x")?,
            "--columns-z" => {
                config.columns_z = Some(parse_required::<u32>(&mut args, "columns-z")?)
            }
            "--chunk-radius" => {
                config.chunk_radius = Some(parse_required::<i32>(&mut args, "chunk-radius")?)
            }
            "--quarter-turns" => {
                config.quarter_turns = parse_required::<u8>(&mut args, "quarter-turns")?
            }
            "--vertical-scale" => {
                config.vertical_scale = parse_required::<f32>(&mut args, "vertical-scale")?
            }
            "--output" => {
                config.output = Some(PathBuf::from(parse_required::<String>(
                    &mut args, "output",
                )?))
            }
            "--stage" => {
                let stage = parse_required::<String>(&mut args, "stage")?;
                if stage != "heightfield" {
                    return Err(cli_error(
                        "heightfield_preview only supports --stage heightfield",
                    ));
                }
            }
            other => {
                return Err(cli_error(format!(
                    "unknown argument '{other}'\n{}",
                    usage()
                )));
            }
        }
    }

    Ok(config)
}

fn parse_required<T>(args: &mut Vec<String>, label: &str) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: Error + Send + Sync + 'static,
{
    let value = args
        .first()
        .cloned()
        .ok_or_else(|| cli_error(format!("missing value for {label}")))?;
    args.remove(0);
    value
        .parse::<T>()
        .map_err(|error| cli_error(format!("invalid {label}: {error}")))
}

fn usage() -> &'static str {
    "usage: cargo run --bin heightfield_preview -- <seed> <center-x> <center-z> [--width <u32>] [--height <u32>] [--world-span-blocks <i32>] [--chunk-radius <i32>] [--columns-x <u32>] [--columns-z <u32>] [--quarter-turns <u8>] [--vertical-scale <f32>] [--output <path>]"
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(CliError(message.into()))
}

#[derive(Debug)]
struct CliError(String);

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for CliError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_output_path_is_short() {
        let config = PreviewConfig {
            seed: 42,
            center_x: 0,
            center_z: 0,
            width: DEFAULT_IMAGE_WIDTH,
            height: DEFAULT_IMAGE_HEIGHT,
            world_span_blocks: DEFAULT_WORLD_SPAN_BLOCKS,
            region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
            land_bias: 0.14,
            columns_x: DEFAULT_COLUMNS_X,
            columns_z: None,
            chunk_radius: None,
            quarter_turns: 0,
            vertical_scale: DEFAULT_VERTICAL_SCALE,
            output: None,
        };

        assert_eq!(
            config.output_path(),
            PathBuf::from("target/heightfield-preview/s42_x0_z0.png")
        );
    }

    #[test]
    fn terrain_ramp_is_not_blank() {
        let low = combined_terrain_ramp(0.15);
        let high = combined_terrain_ramp(0.9);

        assert_ne!(low, high);
        assert!(low[2] > low[0]);
        assert!(high[0] > low[0]);
    }

    #[test]
    fn default_vertical_scale_is_auto_fit_multiplier() {
        assert!((DEFAULT_VERTICAL_SCALE - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn preview_window_reports_chunk_range_and_radius_context() {
        let config = PreviewConfig {
            seed: 42,
            center_x: 0,
            center_z: 0,
            width: DEFAULT_IMAGE_WIDTH,
            height: DEFAULT_IMAGE_HEIGHT,
            world_span_blocks: DEFAULT_WORLD_SPAN_BLOCKS,
            region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
            land_bias: 0.14,
            columns_x: DEFAULT_COLUMNS_X,
            columns_z: None,
            chunk_radius: None,
            quarter_turns: 0,
            vertical_scale: DEFAULT_VERTICAL_SCALE,
            output: None,
        };
        let window = config.window();
        let range = chunk_range_for_window(window);

        assert_eq!(CHUNK_EDGE_I32, 32);
        assert_eq!(PREVIEW_MAJOR_CHUNK_GRID_BLOCKS, 256);
        assert_eq!(MACRO_FIELD_TILE_EDGE_BLOCKS, 1024);
        assert_eq!(range, (-128, 127, -72, 71));
        assert_eq!(nice_scale_blocks(DEFAULT_WORLD_SPAN_BLOCKS), 2048);
    }

    #[test]
    fn preview_grid_spacing_keeps_chunk_major_and_macro_layers_distinct() {
        assert_eq!(
            PREVIEW_MAJOR_CHUNK_GRID_BLOCKS,
            CHUNK_EDGE_I32 * PREVIEW_MAJOR_CHUNK_GRID_MULTIPLIER
        );
        assert_eq!(PREVIEW_MAJOR_CHUNK_GRID_MULTIPLIER, 8);
        assert_eq!(PREVIEW_MAJOR_CHUNK_GRID_BLOCKS, 256);
        assert_eq!(
            MACRO_FIELD_TILE_EDGE_BLOCKS % PREVIEW_MAJOR_CHUNK_GRID_BLOCKS,
            0
        );
    }

    #[test]
    fn parse_chunk_radius_option() {
        let config = parse_args_from([
            "42",
            "64",
            "-33",
            "--chunk-radius",
            "32",
            "--width",
            "1280",
            "--height",
            "720",
        ])
        .expect("parse args");

        assert_eq!(config.chunk_radius, Some(32));
        assert_eq!(config.center_x, 64);
        assert_eq!(config.center_z, -33);
    }

    #[test]
    fn chunk_radius_maps_to_square_chunk_footprint() {
        let config = PreviewConfig {
            seed: 42,
            center_x: 64,
            center_z: -33,
            width: DEFAULT_IMAGE_WIDTH,
            height: DEFAULT_IMAGE_HEIGHT,
            world_span_blocks: DEFAULT_WORLD_SPAN_BLOCKS,
            region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
            land_bias: 0.14,
            columns_x: DEFAULT_COLUMNS_X,
            columns_z: None,
            chunk_radius: Some(2),
            quarter_turns: 0,
            vertical_scale: DEFAULT_VERTICAL_SCALE,
            output: None,
        };
        let window = config.window();
        let range = chunk_range_for_window(window);

        assert_eq!(config.columns_z(), DEFAULT_COLUMNS_X);
        assert_eq!(range, (0, 4, -4, 0));
        assert_eq!(window.world_span_x, 5.0 * CHUNK_EDGE_I32 as f32);
        assert_eq!(window.world_span_z, 5.0 * CHUNK_EDGE_I32 as f32);
        assert_eq!(window.min_x(), 0.0);
        assert_eq!(window.max_x(), 160.0);
        assert_eq!(window.min_z(), -128.0);
        assert_eq!(window.max_z(), 32.0);
    }

    #[test]
    fn overlay_layout_scales_with_image_size() {
        let small = OverlayLayout::new(1280, 720);
        let large = OverlayLayout::new(3840, 2160);

        assert!(large.scale > small.scale);
        assert!(large.panel_width > small.panel_width);
        assert!(large.panel_height > small.panel_height);
        assert!((small.panel_height as f32 / 720.0 - 0.20).abs() < 0.02);
        assert!((large.panel_height as f32 / 2160.0 - 0.20).abs() < 0.02);
    }

    #[test]
    fn iso_plan_default_relief_uses_measurable_image_span() {
        let tile = two_by_two_heightfield_tile();
        let plan = IsoRenderPlan::new(&tile, 1280, 720, 0, DEFAULT_VERTICAL_SCALE)
            .expect("iso render plan");
        let span = plan.projected_height_span_px();

        assert!(span > 720.0 * 0.19);
        assert!(span < 720.0 * 0.36);
        assert!(plan.tile_w_px > 2.0);
    }

    #[test]
    fn cpu_iso_preview_is_nonblank() {
        let tile = two_by_two_heightfield_tile();
        let plan = IsoRenderPlan::new(&tile, 320, 180, 0, DEFAULT_VERTICAL_SCALE)
            .expect("iso render plan");
        let (image, stats) = render_heightfield_isometric(&tile, plan).expect("render");
        let first = image.rgba.chunks_exact(4).next().expect("pixel");
        let varied = image
            .rgba
            .chunks_exact(4)
            .any(|pixel| pixel[0] != first[0] || pixel[1] != first[1] || pixel[2] != first[2]);

        assert!(varied);
        assert!(stats.projected_height_span_px > 0.0);
    }

    fn two_by_two_heightfield_tile() -> HeightfieldTile {
        let columns = vec![
            height_column(0.0, 0.0, -8.0, HeightfieldTerrainKind::Coast),
            height_column(32.0, 0.0, 34.0, HeightfieldTerrainKind::Land),
            height_column(0.0, 32.0, 72.0, HeightfieldTerrainKind::Ridge),
            height_column(32.0, 32.0, 12.0, HeightfieldTerrainKind::River),
        ];
        HeightfieldTile {
            width: 2,
            height: 2,
            sample_spacing_blocks: 32.0,
            columns,
            stats: new_world::world::generation::HeightfieldTileStats {
                column_count: 4,
                min_surface_height_blocks: -8.0,
                max_surface_height_blocks: 72.0,
                average_surface_height_blocks: 27.5,
                max_raw_neighbor_delta_blocks: 80.0,
                max_constrained_neighbor_delta_blocks: 80.0,
                max_snapped_neighbor_delta_blocks: 80.0,
                water_column_count: 0,
                ocean_column_count: 0,
                lake_column_count: 0,
                river_hint_column_count: 1,
                dry_basin_column_count: 0,
                ridge_column_count: 1,
            },
            config: HeightfieldConfig::default(),
        }
    }

    fn height_column(
        x: f32,
        z: f32,
        surface_height_blocks: f32,
        terrain_kind: HeightfieldTerrainKind,
    ) -> HeightfieldColumn {
        HeightfieldColumn {
            position: new_world::world::WorldPlanePoint::new(x, z),
            raw_surface_height_blocks: surface_height_blocks,
            constrained_surface_height_blocks: surface_height_blocks,
            surface_height_blocks,
            surface_y: surface_height_blocks.floor() as i32,
            water_level_blocks: None,
            water_y: None,
            terrain_kind,
            macro_elevation: 0.0,
            combined_macro_height: surface_height_blocks / 160.0,
            ocean_mask: 0.0,
            lake_mask: 0.0,
            dry_basin_mask: 0.0,
            coast_mask: 0.0,
            ridge_influence: if matches!(terrain_kind, HeightfieldTerrainKind::Ridge) {
                1.0
            } else {
                0.0
            },
            river_valley_strength: if matches!(terrain_kind, HeightfieldTerrainKind::River) {
                1.0
            } else {
                0.0
            },
            river_flow_hint: 0.0,
            meso_delta_blocks: 0.0,
            micro_relief_blocks: 0.0,
        }
    }
}
