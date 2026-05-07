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
    BoundaryCache, BoundaryConfig, DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
    DEFAULT_HEIGHTFIELD_MAX_BLOCKS, DEFAULT_HEIGHTFIELD_MIN_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS,
    GraphHydrologyGraph, GraphMacroMap, GraphRegionArea, GraphRegionCoord, HeightfieldColumn,
    HeightfieldConfig, HeightfieldTerrainKind, HeightfieldTile, MacroFieldTile,
    MacroFieldTileConfig, MacroMapConfig, VoronoiGraphConfig, VoronoiGraphPatch,
    VoronoiGraphPatchRequest, generate_heightfield_tile, generate_macro_field_tile,
    generate_macro_map, generate_noisy_boundaries, generate_voronoi_graph_patch,
    graph_region_for_world_block, solve_hydrology,
};

mod common;

const DEFAULT_IMAGE_WIDTH: u32 = 1280;
const DEFAULT_IMAGE_HEIGHT: u32 = 720;
const DEFAULT_WORLD_SPAN_BLOCKS: i32 = 8192;
const DEFAULT_COLUMNS_X: u32 = 192;
const HEIGHTFIELD_PREVIEW_XZ_SCALE: u32 = 2;
const WATER_ALPHA: f32 = 0.72;
const ISO_TILE_HEIGHT_RATIO: f32 = 0.50;
const MACRO_FIELD_TILE_EDGE_BLOCKS: i32 = DEFAULT_GRAPH_REGION_SIZE_BLOCKS;
const PREVIEW_MAJOR_CHUNK_GRID_MULTIPLIER: i32 = 8;
const PREVIEW_MAJOR_CHUNK_GRID_BLOCKS: i32 = CHUNK_EDGE_I32 * PREVIEW_MAJOR_CHUNK_GRID_MULTIPLIER;
const DEFAULT_BLOCK_LINES: bool = true;

#[derive(Debug, Clone)]
struct PreviewConfig {
    seed: u64,
    center_x: i32,
    center_z: i32,
    center_is_world_blocks: bool,
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
    block_lines: bool,
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
        let columns_z = self.columns_z();
        self.columns_x
            .checked_mul(HEIGHTFIELD_PREVIEW_XZ_SCALE)
            .ok_or_else(|| cli_error("columns-x * fixed xz-scale is too large"))?;
        columns_z
            .checked_mul(HEIGHTFIELD_PREVIEW_XZ_SCALE)
            .ok_or_else(|| cli_error("columns-z * fixed xz-scale is too large"))?;
        if self.chunk_radius.is_some_and(|radius| radius < 0) {
            return Err(cli_error("chunk-radius must be zero or positive"));
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

    fn effective_columns_x(&self) -> u32 {
        self.columns_x
            .checked_mul(HEIGHTFIELD_PREVIEW_XZ_SCALE)
            .expect("validated fixed xz-scale and columns-x")
    }

    fn effective_columns_z(&self) -> u32 {
        self.columns_z()
            .checked_mul(HEIGHTFIELD_PREVIEW_XZ_SCALE)
            .expect("validated fixed xz-scale and columns-z")
    }

    fn window(&self) -> PreviewWindow {
        let center_chunk_x = self.center_chunk_x();
        let center_chunk_z = self.center_chunk_z();
        let (center_x, center_z, span_x, span_z) = if let Some(radius) = self.chunk_radius {
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
            (
                self.center_world_x() as f32,
                self.center_world_z() as f32,
                span_x,
                span_z,
            )
        };
        PreviewWindow {
            center_x,
            center_z,
            base_columns_x: self.columns_x,
            base_columns_z: self.columns_z(),
            xz_scale: HEIGHTFIELD_PREVIEW_XZ_SCALE,
            columns_x: self.effective_columns_x(),
            columns_z: self.effective_columns_z(),
            world_span_x: span_x,
            world_span_z: span_z,
        }
    }

    fn output_path(&self) -> PathBuf {
        self.output.clone().unwrap_or_else(|| {
            let radius = self.output_radius_suffix();
            PathBuf::from(format!(
                "target/heightfield-preview/s{}_cx{}_cz{}_q{}_{}.png",
                self.seed,
                self.center_x,
                self.center_z,
                self.quarter_turns % 4,
                radius
            ))
        })
    }

    fn output_radius_suffix(&self) -> String {
        if let Some(radius) = self.chunk_radius {
            return format!("r{radius}");
        }
        let window = self.window();
        let range = chunk_range_for_window(window);
        let center_chunk_x = self.center_chunk_x();
        let center_chunk_z = self.center_chunk_z();
        let radius_x = (center_chunk_x - range.0)
            .abs()
            .max((range.1 - center_chunk_x).abs());
        let radius_z = (center_chunk_z - range.2)
            .abs()
            .max((range.3 - center_chunk_z).abs());
        if radius_x == radius_z {
            format!("r{radius_x}")
        } else {
            format!("r{radius_x}x{radius_z}")
        }
    }

    fn center_chunk_x(&self) -> i32 {
        if self.center_is_world_blocks {
            self.center_x.div_euclid(CHUNK_EDGE_I32)
        } else {
            self.center_x
        }
    }

    fn center_chunk_z(&self) -> i32 {
        if self.center_is_world_blocks {
            self.center_z.div_euclid(CHUNK_EDGE_I32)
        } else {
            self.center_z
        }
    }

    fn center_world_x(&self) -> i32 {
        if self.center_is_world_blocks {
            self.center_x
        } else {
            self.center_x * CHUNK_EDGE_I32 + CHUNK_EDGE_I32 / 2
        }
    }

    fn center_world_z(&self) -> i32 {
        if self.center_is_world_blocks {
            self.center_z
        } else {
            self.center_z * CHUNK_EDGE_I32 + CHUNK_EDGE_I32 / 2
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PreviewWindow {
    center_x: f32,
    center_z: f32,
    base_columns_x: u32,
    base_columns_z: u32,
    xz_scale: u32,
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

    fn base_sample_spacing(self) -> f32 {
        self.world_span_x / self.base_columns_x as f32
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
    input_center_x: i32,
    input_center_z: i32,
    center_chunk_x: i32,
    center_chunk_z: i32,
    center_world_x: i32,
    center_world_z: i32,
    center_is_world_blocks: bool,
    width: u32,
    height: u32,
    world_span_blocks: i32,
    world_min_x: f32,
    world_max_x: f32,
    world_min_z: f32,
    world_max_z: f32,
    base_columns_x: u32,
    base_columns_z: u32,
    xz_scale: u32,
    columns_x: u32,
    columns_z: u32,
    base_sample_spacing_blocks: f32,
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
    graph_site_count: usize,
    macro_sample_count: usize,
    column_count: usize,
    min_surface: f32,
    avg_surface: f32,
    max_surface: f32,
    max_raw_neighbor_delta: f32,
    max_contour_guided_neighbor_delta: f32,
    max_constrained_neighbor_delta: f32,
    max_snapped_neighbor_delta: f32,
    max_visible_neighbor_delta: f32,
    max_shore_visible_neighbor_delta: f32,
    min_ocean_visible_surface: f32,
    max_ocean_visible_surface: f32,
    min_land_near_water_surface: f32,
    max_river_water_neighbor_delta: f32,
    river_uphill_flow_neighbors: usize,
    contour_step_blocks: f32,
    contour_min_gap_blocks: f32,
    contour_river_min_gap_blocks: f32,
    contour_band_smoothing: f32,
    vertical_px_per_block: f32,
    projected_height_span_px: f32,
    quarter_turns: u8,
    block_lines: bool,
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
            format!(
                "input_center={},{}",
                self.input_center_x, self.input_center_z
            ),
            format!(
                "input_center_units={}",
                if self.center_is_world_blocks {
                    "world_blocks_compat"
                } else {
                    "chunk_coordinates"
                }
            ),
            format!(
                "center_chunk={},{}",
                self.center_chunk_x, self.center_chunk_z
            ),
            format!(
                "center_world_blocks={},{}",
                self.center_world_x, self.center_world_z
            ),
            format!("image={}x{}", self.width, self.height),
            format!(
                "orientation_overlay=heightfield_projected_compass_quarter_turns_{}",
                self.quarter_turns
            ),
            format!("world_span_blocks={}", self.world_span_blocks),
            format!(
                "world_footprint_blocks=x:{:.1}..{:.1},z:{:.1}..{:.1}",
                self.world_min_x, self.world_max_x, self.world_min_z, self.world_max_z
            ),
            format!(
                "base_columns={}x{}",
                self.base_columns_x, self.base_columns_z
            ),
            format!("xz_scale={}_fixed", self.xz_scale),
            format!("horizontal_subdivisions={}_fixed", self.xz_scale),
            format!("effective_columns={}x{}", self.columns_x, self.columns_z),
            format!(
                "base_sample_spacing_blocks={:.3}",
                self.base_sample_spacing_blocks
            ),
            format!(
                "effective_sample_spacing_blocks={:.3}",
                self.sample_spacing_blocks
            ),
            "height_values=relief_compressed_before_preview".to_string(),
            "render_scale_policy=cubic_block_pixels_no_vertical_normalization".to_string(),
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
            format!(
                "grid_overlay=primary_macro_tile_{}blocks_secondary_major_{}blocks_faint_chunk_{}blocks",
                self.macro_tile_edge_blocks,
                self.major_grid_edge_blocks,
                self.chunk_edge_blocks
            ),
            "view=cpu_isometric_columns".to_string(),
            format!(
                "projection=screen_x_(x-z)*tile_w/2_screen_y_(x+z)*tile_h/2-y*vertical_px_quarter_turns_{}",
                self.quarter_turns
            ),
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
                "neighbor_delta_raw_contour_constrained_snapped_visible_shore_visible_blocks={:.3},{:.3},{:.3},{:.3},{:.3},{:.3}",
                self.max_raw_neighbor_delta,
                self.max_contour_guided_neighbor_delta,
                self.max_constrained_neighbor_delta,
                self.max_snapped_neighbor_delta,
                self.max_visible_neighbor_delta,
                self.max_shore_visible_neighbor_delta
            ),
            format!(
                "waterline_ocean_visible_min_max_land_near_water_min={:.3},{:.3},{:.3}",
                self.min_ocean_visible_surface,
                self.max_ocean_visible_surface,
                self.min_land_near_water_surface
            ),
            format!(
                "river_water_delta_max_uphill_flow_neighbors={:.3},{}",
                self.max_river_water_neighbor_delta,
                self.river_uphill_flow_neighbors
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
            contour_gap_metadata(
                self.contour_step_blocks,
                self.contour_min_gap_blocks,
                self.contour_river_min_gap_blocks,
                self.contour_band_smoothing,
            ),
            "height_snap=round_to_integer_block".to_string(),
            format!("block_lines={}", self.block_lines),
            "water_policy=ocean_lake_visible_surface_y0_no_preview_bathymetry_river_integer_descent".to_string(),
            format!(
                "height_mapping=signed_combined_macro_height_-0.75_to_0_to_1.25_maps_{:.0}_to_0_to_{:.0}_blocks",
                DEFAULT_HEIGHTFIELD_MIN_BLOCKS,
                DEFAULT_HEIGHTFIELD_MAX_BLOCKS
            ),
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

fn contour_gaps_are_unified(land_gap_blocks: f32, river_gap_blocks: f32) -> bool {
    (land_gap_blocks - river_gap_blocks).abs() <= 0.001
}

fn contour_gap_metadata(
    step_blocks: f32,
    land_gap_blocks: f32,
    river_gap_blocks: f32,
    band_smoothing: f32,
) -> String {
    if contour_gaps_are_unified(land_gap_blocks, river_gap_blocks) {
        format!(
            "contour_band_heightfield=step:{step_blocks:.2}_blocks,unified_land_river_min_gap:{land_gap_blocks:.2}_blocks,smoothing_disabled:{band_smoothing:.2}"
        )
    } else {
        format!(
            "contour_band_heightfield=step:{step_blocks:.2}_blocks,land_min_gap:{land_gap_blocks:.2}_blocks,river_min_gap:{river_gap_blocks:.2}_blocks,smoothing_disabled:{band_smoothing:.2}"
        )
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
    let heightfield = generate_heightfield_tile(
        &macro_tile,
        HeightfieldConfig {
            horizontal_subdivisions: HEIGHTFIELD_PREVIEW_XZ_SCALE,
            ..HeightfieldConfig::default()
        },
    );
    let heightfield_ms = heightfield_start.elapsed().as_millis();

    let mesh_start = Instant::now();
    let plan = IsoRenderPlan::new(
        &heightfield,
        config.width,
        config.height,
        config.quarter_turns % 4,
    )?;
    let mesh_ms = mesh_start.elapsed().as_millis();

    let render_start = Instant::now();
    let (mut image, iso_stats) =
        render_heightfield_isometric(&heightfield, plan, config.block_lines)?;
    let render_ms = render_start.elapsed().as_millis();

    let total_ms = total_start.elapsed().as_millis();
    let chunk_range = chunk_range_for_window(window);
    let center_chunk_x = config.center_chunk_x();
    let center_chunk_z = config.center_chunk_z();
    let center_world_x = config.center_world_x();
    let center_world_z = config.center_world_z();
    let header = PreviewHeader {
        seed: config.seed,
        generator_version: meta.generator_version,
        input_center_x: config.center_x,
        input_center_z: config.center_z,
        center_chunk_x,
        center_chunk_z,
        center_world_x,
        center_world_z,
        center_is_world_blocks: config.center_is_world_blocks,
        width: config.width,
        height: config.height,
        world_span_blocks: window.world_span_x.round() as i32,
        world_min_x: window.min_x(),
        world_max_x: window.max_x(),
        world_min_z: window.min_z(),
        world_max_z: window.max_z(),
        base_columns_x: window.base_columns_x,
        base_columns_z: window.base_columns_z,
        xz_scale: window.xz_scale,
        columns_x: window.columns_x,
        columns_z: window.columns_z,
        base_sample_spacing_blocks: window.base_sample_spacing(),
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
        graph_site_count: patch.sites.len(),
        macro_sample_count: macro_tile.samples.len(),
        column_count: heightfield.stats.column_count,
        min_surface: heightfield.stats.min_surface_height_blocks,
        avg_surface: heightfield.stats.average_surface_height_blocks,
        max_surface: heightfield.stats.max_surface_height_blocks,
        max_raw_neighbor_delta: heightfield.stats.max_raw_neighbor_delta_blocks,
        max_contour_guided_neighbor_delta: heightfield
            .stats
            .max_contour_guided_neighbor_delta_blocks,
        max_constrained_neighbor_delta: heightfield.stats.max_constrained_neighbor_delta_blocks,
        max_snapped_neighbor_delta: heightfield.stats.max_snapped_neighbor_delta_blocks,
        max_visible_neighbor_delta: heightfield.stats.max_visible_neighbor_delta_blocks,
        max_shore_visible_neighbor_delta: heightfield.stats.max_shore_visible_neighbor_delta_blocks,
        min_ocean_visible_surface: heightfield.stats.min_ocean_visible_surface_blocks,
        max_ocean_visible_surface: heightfield.stats.max_ocean_visible_surface_blocks,
        min_land_near_water_surface: heightfield.stats.min_land_near_water_surface_blocks,
        max_river_water_neighbor_delta: heightfield.stats.max_river_water_neighbor_delta_blocks,
        river_uphill_flow_neighbors: heightfield.stats.river_uphill_flow_neighbor_count,
        contour_step_blocks: heightfield.stats.contour_step_blocks,
        contour_min_gap_blocks: heightfield.stats.contour_min_gap_blocks,
        contour_river_min_gap_blocks: heightfield.stats.contour_river_min_gap_blocks,
        contour_band_smoothing: heightfield.stats.contour_band_smoothing,
        vertical_px_per_block: iso_stats.vertical_px_per_block,
        projected_height_span_px: iso_stats.projected_height_span_px,
        quarter_turns: config.quarter_turns % 4,
        block_lines: config.block_lines,
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
    draw_iso_compass_offscreen(&mut image, plan);
    write_rgba_png_with_metadata(&image, &output, &header)?;

    println!("heightfield preview: seed {}", config.seed);
    println!(
        "window: center chunk=({}, {}), center world=({}, {}), span={:.0}x{:.0} blocks, base columns={}x{}, fixed xz scale={}x, effective columns={}x{}, spacing {:.2}->{:.2} blocks",
        center_chunk_x,
        center_chunk_z,
        center_world_x,
        center_world_z,
        window.world_span_x,
        window.world_span_z,
        window.base_columns_x,
        window.base_columns_z,
        window.xz_scale,
        window.columns_x,
        window.columns_z,
        window.base_sample_spacing(),
        window.sample_spacing()
    );
    if config.center_is_world_blocks {
        println!(
            "compat input: positional center was read as world blocks ({}, {})",
            config.center_x, config.center_z
        );
    } else {
        println!(
            "input: positional center is chunk coordinates ({}, {})",
            config.center_x, config.center_z
        );
    }
    println!(
        "block lines: {}",
        if config.block_lines { "on" } else { "off" }
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
        "grid overlay: primary macro tile {} blocks, secondary major {} blocks, chunk {} blocks very faint",
        MACRO_FIELD_TILE_EDGE_BLOCKS, PREVIEW_MAJOR_CHUNK_GRID_BLOCKS, CHUNK_EDGE_I32
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
        "view: cpu isometric columns, quarter turns {}, cubic block render scale, vertical {:.3} px/block, relief span {:.1}px",
        config.quarter_turns % 4,
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
        "neighbor delta raw/contour/constrained/snapped/visible/shore-visible max {:.2}/{:.2}/{:.2}/{:.2}/{:.2}/{:.2} blocks",
        heightfield.stats.max_raw_neighbor_delta_blocks,
        heightfield.stats.max_contour_guided_neighbor_delta_blocks,
        heightfield.stats.max_constrained_neighbor_delta_blocks,
        heightfield.stats.max_snapped_neighbor_delta_blocks,
        header.max_visible_neighbor_delta,
        header.max_shore_visible_neighbor_delta
    );
    println!(
        "waterline: ocean visible min/max {:.2}/{:.2}, land near water min {:.2}, river water neighbor delta max {:.2}, uphill-flow neighbors {}",
        header.min_ocean_visible_surface,
        header.max_ocean_visible_surface,
        header.min_land_near_water_surface,
        header.max_river_water_neighbor_delta,
        header.river_uphill_flow_neighbors
    );
    if contour_gaps_are_unified(
        heightfield.stats.contour_min_gap_blocks,
        heightfield.stats.contour_river_min_gap_blocks,
    ) {
        println!(
            "contour-band heightfield: step {:.1} blocks, unified land/river gap {:.1} blocks, smoothing disabled {:.2}",
            heightfield.stats.contour_step_blocks,
            heightfield.stats.contour_min_gap_blocks,
            heightfield.stats.contour_band_smoothing
        );
    } else {
        println!(
            "contour-band heightfield: step {:.1} blocks, land gap {:.1} blocks, river gap {:.1} blocks, smoothing disabled {:.2}",
            heightfield.stats.contour_step_blocks,
            heightfield.stats.contour_min_gap_blocks,
            heightfield.stats.contour_river_min_gap_blocks,
            heightfield.stats.contour_band_smoothing
        );
    }
    println!(
        "xz scale: {}x fixed horizontal columns; horizontal subdivisions are not user-configurable; y relief is compressed in heightfield block-domain before cubic preview rendering",
        HEIGHTFIELD_PREVIEW_XZ_SCALE
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
    ) -> Result<Self, Box<dyn Error>> {
        if tile.columns.is_empty() {
            return Err(cli_error("heightfield preview cannot render an empty tile"));
        }
        let min_surface = tile.stats.min_surface_height_blocks;
        let max_surface = tile.stats.max_surface_height_blocks;
        let height_range = (max_surface - min_surface).max(1.0);
        let footprint_axis_count = (tile.width + tile.height).max(2) as f32;
        let tile_w_by_width = width as f32 * 1.64 / footprint_axis_count;
        let height_budget = height as f32 * 0.86;
        let height_denominator =
            ISO_TILE_HEIGHT_RATIO * (footprint_axis_count * 0.5 + height_range);
        let tile_w_by_height = height_budget / height_denominator.max(f32::EPSILON);
        let tile_w_px = tile_w_by_width.min(tile_w_by_height).clamp(2.0, 24.0);
        let tile_h_px = tile_w_px * ISO_TILE_HEIGHT_RATIO;
        let vertical_px_per_block = tile_h_px;
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
        let (rx, rz) = rotate_grid_delta(self.quarter_turns, dx, dz);
        Point2 {
            x: (rx - rz) * self.tile_w_px * 0.5 + self.offset_x_px,
            y: (rx + rz) * self.tile_h_px * 0.5 - y_blocks * self.vertical_px_per_block
                + self.offset_y_px,
        }
    }

    fn horizontal_depth_key(self, x: f32, z: f32, tile: &HeightfieldTile) -> f32 {
        let center_x = tile.width as f32 * 0.5;
        let center_z = tile.height as f32 * 0.5;
        let (rx, rz) = rotate_grid_delta(self.quarter_turns, x - center_x, z - center_z);
        (rx + rz) * self.tile_h_px * 0.5
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

fn rotate_grid_delta(quarter_turns: u8, dx: f32, dz: f32) -> (f32, f32) {
    match quarter_turns % 4 {
        0 => (dx, dz),
        1 => (dz, -dx),
        2 => (-dx, -dz),
        3 => (-dz, dx),
        _ => unreachable!(),
    }
}

fn iso_cardinal_screen_delta(quarter_turns: u8, dx: f32, dz: f32) -> Point2 {
    let (rx, rz) = rotate_grid_delta(quarter_turns, dx, dz);
    Point2 {
        x: (rx - rz) * 0.5,
        y: (rx + rz) * 0.5,
    }
}

fn render_heightfield_isometric(
    tile: &HeightfieldTile,
    plan: IsoRenderPlan,
    block_lines: bool,
) -> Result<(OffscreenRenderOutput, IsoRenderStats), Box<dyn Error>> {
    let mut image = RgbaImage::from_pixel(plan.width, plan.height, image::Rgba([12, 15, 18, 255]));
    let width = tile.width as usize;
    let height = tile.height as usize;
    let mut draw_order = (0..height)
        .flat_map(|z| {
            (0..width).map(move |x| {
                let depth = plan.horizontal_depth_key(x as f32 + 0.5, z as f32 + 0.5, tile);
                (depth, x, z)
            })
        })
        .collect::<Vec<_>>();
    draw_order.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| a.2.cmp(&b.2))
            .then_with(|| a.1.cmp(&b.1))
    });

    for (_, x, z) in draw_order {
        let index = z * width + x;
        draw_column_iso(
            &mut image,
            tile,
            plan,
            x,
            z,
            tile.columns[index],
            block_lines,
        );
    }
    Ok((
        OffscreenRenderOutput {
            width: plan.width,
            height: plan.height,
            rgba: image.into_raw(),
            draw_call_count: (tile.columns.len() * if block_lines { 5 } else { 3 }) as u32,
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
    block_lines: bool,
) {
    let surface = column.surface_height_blocks;
    let visible_surface = visible_surface_height(column);
    let color = terrain_color_rgba(column);
    for side in visible_side_directions(plan.quarter_turns) {
        let neighbor_height = neighbor_visible_surface(tile, x, z, side.dx, side.dz)
            .unwrap_or(visible_surface - 12.0);
        if visible_surface > neighbor_height + 0.75 {
            draw_side_face(
                image,
                tile,
                plan,
                side_edge(x, z, side.dx, side.dz),
                neighbor_height,
                visible_surface,
                shade_rgba(color, side.shade),
                block_lines,
            );
        }
    }
    let top_surface = if matches!(
        column.terrain_kind,
        HeightfieldTerrainKind::Ocean | HeightfieldTerrainKind::Lake
    ) {
        visible_surface
    } else {
        surface
    };
    draw_top_face(
        image,
        tile,
        plan,
        x,
        z,
        top_surface,
        shade_rgba(color, 1.05),
        block_lines,
    );

    if let Some(water) = column.water_level_blocks {
        if water >= surface {
            draw_top_face(
                image,
                tile,
                plan,
                x,
                z,
                water + 0.10,
                water_color_rgba(column),
                block_lines,
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct VisibleSide {
    dx: isize,
    dz: isize,
    shade: f32,
}

fn visible_side_directions(quarter_turns: u8) -> [VisibleSide; 2] {
    let mut sides = [
        VisibleSide {
            dx: 1,
            dz: 0,
            shade: 0.68,
        },
        VisibleSide {
            dx: -1,
            dz: 0,
            shade: 0.62,
        },
        VisibleSide {
            dx: 0,
            dz: 1,
            shade: 0.56,
        },
        VisibleSide {
            dx: 0,
            dz: -1,
            shade: 0.60,
        },
    ];
    sides.sort_by(|a, b| {
        iso_cardinal_screen_delta(quarter_turns, a.dx as f32, a.dz as f32)
            .y
            .total_cmp(&iso_cardinal_screen_delta(quarter_turns, b.dx as f32, b.dz as f32).y)
            .reverse()
    });
    [sides[0], sides[1]]
}

fn neighbor_visible_surface(
    tile: &HeightfieldTile,
    x: usize,
    z: usize,
    dx: isize,
    dz: isize,
) -> Option<f32> {
    let nx = x.checked_add_signed(dx)?;
    let nz = z.checked_add_signed(dz)?;
    if nx >= tile.width as usize || nz >= tile.height as usize {
        return None;
    }
    Some(visible_surface_height(
        tile.columns[nz * tile.width as usize + nx],
    ))
}

fn side_edge(x: usize, z: usize, dx: isize, dz: isize) -> [f32; 4] {
    match (dx, dz) {
        (1, 0) => [(x + 1) as f32, z as f32, (x + 1) as f32, (z + 1) as f32],
        (-1, 0) => [x as f32, z as f32, x as f32, (z + 1) as f32],
        (0, 1) => [x as f32, (z + 1) as f32, (x + 1) as f32, (z + 1) as f32],
        (0, -1) => [x as f32, z as f32, (x + 1) as f32, z as f32],
        _ => unreachable!("side directions are cardinal"),
    }
}

fn visible_surface_height(column: HeightfieldColumn) -> f32 {
    column.visible_surface_height_blocks()
}

fn draw_top_face(
    image: &mut RgbaImage,
    tile: &HeightfieldTile,
    plan: IsoRenderPlan,
    x: usize,
    z: usize,
    y: f32,
    color: [u8; 4],
    block_lines: bool,
) {
    let polygon = [
        plan.project_grid(x as f32, z as f32, y, tile),
        plan.project_grid((x + 1) as f32, z as f32, y, tile),
        plan.project_grid((x + 1) as f32, (z + 1) as f32, y, tile),
        plan.project_grid(x as f32, (z + 1) as f32, y, tile),
    ];
    fill_convex_polygon(image, &polygon, color);
    if block_lines {
        draw_polygon_outline(image, &polygon, [4, 8, 10, 42], 1);
    }
}

fn draw_side_face(
    image: &mut RgbaImage,
    tile: &HeightfieldTile,
    plan: IsoRenderPlan,
    edge: [f32; 4],
    lower_y: f32,
    upper_y: f32,
    color: [u8; 4],
    block_lines: bool,
) {
    let lower_y = lower_y.max(upper_y - 96.0);
    let polygon = [
        plan.project_grid(edge[0], edge[1], upper_y, tile),
        plan.project_grid(edge[2], edge[3], upper_y, tile),
        plan.project_grid(edge[2], edge[3], lower_y, tile),
        plan.project_grid(edge[0], edge[1], lower_y, tile),
    ];
    fill_convex_polygon(image, &polygon, color);
    if block_lines {
        draw_polygon_outline(image, &polygon, [2, 5, 7, 34], 1);
    }
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
        [20, 27, 33, 18],
        6,
    );
    draw_world_grid_overlay(
        &mut rgba,
        tile,
        plan,
        window,
        PREVIEW_MAJOR_CHUNK_GRID_BLOCKS,
        [70, 91, 104, 38],
        4,
    );
    draw_world_grid_overlay(
        &mut rgba,
        tile,
        plan,
        window,
        MACRO_FIELD_TILE_EDGE_BLOCKS,
        [226, 236, 246, 132],
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

fn draw_polygon_outline(image: &mut RgbaImage, points: &[Point2], color: [u8; 4], thickness: i32) {
    if points.len() < 2 {
        return;
    }
    for i in 0..points.len() {
        let start = points[i];
        let end = points[(i + 1) % points.len()];
        draw_projected_line_thick(image, start, end, color, 1, thickness);
    }
}

fn draw_projected_line_thick(
    image: &mut RgbaImage,
    start: Point2,
    end: Point2,
    color: [u8; 4],
    dash_period: i32,
    thickness: i32,
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
        for oy in -thickness / 2..=thickness / 2 {
            for ox in -thickness / 2..=thickness / 2 {
                if x + ox < 0
                    || y + oy < 0
                    || x + ox >= image.width() as i32
                    || y + oy >= image.height() as i32
                {
                    continue;
                }
                blend_rgba(
                    image,
                    (x + ox) as u32,
                    (y + oy) as u32,
                    color,
                    color[3] as f32 / 255.0,
                );
            }
        }
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
    let altitude = (column.surface_height_blocks / DEFAULT_HEIGHTFIELD_MAX_BLOCKS).clamp(-0.2, 0.6);
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
        &format!("CCH {} {}", header.center_chunk_x, header.center_chunk_z),
        [204, 214, 203, 255],
        layout.scale,
    );
    text_y += layout.line_step;
    draw_text(
        &mut rgba,
        text_x,
        text_y,
        &format!("W {} {}", header.center_world_x, header.center_world_z),
        [204, 214, 203, 255],
        layout.scale,
    );
    text_y += layout.line_step;
    draw_text(
        &mut rgba,
        text_x,
        text_y,
        &format!(
            "COL {}X{} XZ{}F",
            header.columns_x, header.columns_z, header.xz_scale
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
            "STEP {:.0}/{:.0}",
            header.base_sample_spacing_blocks, header.sample_spacing_blocks
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
        "CUBE 1:1:1 RELIEF50",
        [204, 214, 203, 255],
        layout.scale,
    );
    text_y += layout.line_step;
    let band_label = if contour_gaps_are_unified(
        header.contour_min_gap_blocks,
        header.contour_river_min_gap_blocks,
    ) {
        format!(
            "BAND {:.0}B GAP {:.0}B",
            header.contour_step_blocks, header.contour_min_gap_blocks
        )
    } else {
        format!(
            "BAND {:.0}B LAND GAP {:.0}B RIV {:.0}B",
            header.contour_step_blocks,
            header.contour_min_gap_blocks,
            header.contour_river_min_gap_blocks
        )
    };
    draw_text(
        &mut rgba,
        text_x,
        text_y,
        &band_label,
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

fn draw_iso_compass_offscreen(image: &mut OffscreenRenderOutput, plan: IsoRenderPlan) {
    let Some(mut rgba) =
        RgbaImage::from_raw(image.width, image.height, std::mem::take(&mut image.rgba))
    else {
        return;
    };
    draw_iso_compass_rgba(&mut rgba, plan.quarter_turns);
    image.rgba = rgba.into_raw();
}

fn draw_iso_compass_rgba(image: &mut RgbaImage, quarter_turns: u8) {
    let layout = OverlayLayout::new(image.width(), image.height());
    let panel = (layout.panel_height.min(layout.panel_width) / 2).max(26 * layout.scale);
    let margin = layout.margin;
    let x = image.width().saturating_sub(panel + margin);
    let y = margin;
    let center = Point2 {
        x: x as f32 + panel as f32 * 0.5,
        y: y as f32 + panel as f32 * 0.5,
    };
    let arm = (panel as f32 * 0.34).max(12.0);
    draw_panel(image, x, y, panel, panel);

    for (label, dx, dz, color) in [
        ("N", 0.0, -1.0, [232, 238, 226, 255]),
        ("E", 1.0, 0.0, [122, 196, 238, 255]),
        ("S", 0.0, 1.0, [182, 193, 184, 255]),
        ("W", -1.0, 0.0, [182, 193, 184, 255]),
    ] {
        let dir = normalized_point(iso_cardinal_screen_delta(quarter_turns, dx, dz));
        let tip = Point2 {
            x: center.x + dir.x * arm,
            y: center.y + dir.y * arm,
        };
        draw_projected_line_thick(image, center, tip, color, 1, 2);
        draw_compass_arrow_head(image, tip, dir, color);
        let label_pos = Point2 {
            x: center.x + dir.x * (arm + 8.0 * layout.scale as f32),
            y: center.y + dir.y * (arm + 8.0 * layout.scale as f32),
        };
        draw_text(
            image,
            label_pos.x.round().max(0.0) as u32,
            label_pos.y.round().max(0.0) as u32,
            label,
            color,
            layout.scale,
        );
    }
}

fn normalized_point(point: Point2) -> Point2 {
    let length = (point.x * point.x + point.y * point.y)
        .sqrt()
        .max(f32::EPSILON);
    Point2 {
        x: point.x / length,
        y: point.y / length,
    }
}

fn draw_compass_arrow_head(image: &mut RgbaImage, tip: Point2, dir: Point2, color: [u8; 4]) {
    let len = (image.width().min(image.height()) as f32 / 70.0).clamp(6.0, 18.0);
    let wing = (len * 0.52).max(3.0);
    let base = Point2 {
        x: tip.x - dir.x * len,
        y: tip.y - dir.y * len,
    };
    let perp = Point2 {
        x: -dir.y,
        y: dir.x,
    };
    draw_projected_line_thick(
        image,
        tip,
        Point2 {
            x: base.x + perp.x * wing,
            y: base.y + perp.y * wing,
        },
        color,
        1,
        2,
    );
    draw_projected_line_thick(
        image,
        tip,
        Point2 {
            x: base.x - perp.x * wing,
            y: base.y - perp.y * wing,
        },
        color,
        1,
        2,
    );
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
        (
            format!("T{}B", header.macro_tile_edge_blocks),
            [226, 236, 246, 255],
        ),
        (
            format!("G{}B", header.major_grid_edge_blocks),
            [70, 91, 104, 255],
        ),
        (format!("C{}B", header.chunk_edge_blocks), [20, 27, 33, 255]),
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
        center_is_world_blocks: false,
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
        block_lines: DEFAULT_BLOCK_LINES,
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
            "--world-center" | "--world-coordinates" => {
                config.center_is_world_blocks = true;
            }
            "--block-lines" => {
                config.block_lines = true;
            }
            "--no-block-lines" => {
                config.block_lines = false;
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
    "usage: cargo run --bin heightfield_preview -- <seed> <center-chunk-x> <center-chunk-z> [--world-center] [--width <u32>] [--height <u32>] [--world-span-blocks <i32>] [--chunk-radius <i32>] [--columns-x <u32>] [--columns-z <u32>] [--quarter-turns <u8>] [--block-lines|--no-block-lines] [--output <path>] (fixed xz scale 2)"
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
            center_is_world_blocks: false,
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
            block_lines: DEFAULT_BLOCK_LINES,
            output: None,
        };

        assert_eq!(
            config.output_path(),
            PathBuf::from("target/heightfield-preview/s42_cx0_cz0_q0_r128x72.png")
        );
    }

    #[test]
    fn default_chunk_radius_output_path_is_short() {
        let config = PreviewConfig {
            seed: 42,
            center_x: 0,
            center_z: 0,
            center_is_world_blocks: false,
            width: DEFAULT_IMAGE_WIDTH,
            height: DEFAULT_IMAGE_HEIGHT,
            world_span_blocks: DEFAULT_WORLD_SPAN_BLOCKS,
            region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
            land_bias: 0.14,
            columns_x: DEFAULT_COLUMNS_X,
            columns_z: None,
            chunk_radius: Some(4),
            quarter_turns: 2,
            block_lines: DEFAULT_BLOCK_LINES,
            output: None,
        };

        assert_eq!(
            config.output_path(),
            PathBuf::from("target/heightfield-preview/s42_cx0_cz0_q2_r4.png")
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
    fn preview_window_reports_chunk_range_and_radius_context() {
        let config = PreviewConfig {
            seed: 42,
            center_x: 0,
            center_z: 0,
            center_is_world_blocks: false,
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
            block_lines: DEFAULT_BLOCK_LINES,
            output: None,
        };
        let window = config.window();
        let range = chunk_range_for_window(window);

        assert_eq!(CHUNK_EDGE_I32, 32);
        assert_eq!(PREVIEW_MAJOR_CHUNK_GRID_BLOCKS, 256);
        assert_eq!(MACRO_FIELD_TILE_EDGE_BLOCKS, 1024);
        assert_eq!(window.base_columns_x, DEFAULT_COLUMNS_X);
        assert_eq!(
            window.columns_x,
            DEFAULT_COLUMNS_X * HEIGHTFIELD_PREVIEW_XZ_SCALE
        );
        assert_eq!(
            window.sample_spacing(),
            window.base_sample_spacing() / HEIGHTFIELD_PREVIEW_XZ_SCALE as f32
        );
        assert_eq!(range, (-128, 128, -72, 72));
        assert_eq!(nice_scale_blocks(DEFAULT_WORLD_SPAN_BLOCKS), 2048);
    }

    #[test]
    fn preview_grid_spacing_uses_macro_tile_as_primary_scale() {
        assert_eq!(
            PREVIEW_MAJOR_CHUNK_GRID_BLOCKS,
            CHUNK_EDGE_I32 * PREVIEW_MAJOR_CHUNK_GRID_MULTIPLIER
        );
        assert_eq!(PREVIEW_MAJOR_CHUNK_GRID_MULTIPLIER, 8);
        assert_eq!(PREVIEW_MAJOR_CHUNK_GRID_BLOCKS, 256);
        assert_eq!(MACRO_FIELD_TILE_EDGE_BLOCKS, 1024);
        assert_eq!(
            MACRO_FIELD_TILE_EDGE_BLOCKS % PREVIEW_MAJOR_CHUNK_GRID_BLOCKS,
            0
        );
        assert_eq!(
            MACRO_FIELD_TILE_EDGE_BLOCKS / PREVIEW_MAJOR_CHUNK_GRID_BLOCKS,
            4
        );
    }

    #[test]
    fn parse_chunk_radius_option() {
        let config = parse_args_from([
            "42",
            "2",
            "-3",
            "--chunk-radius",
            "32",
            "--width",
            "1280",
            "--height",
            "720",
        ])
        .expect("parse args");

        assert_eq!(config.chunk_radius, Some(32));
        assert_eq!(config.center_x, 2);
        assert_eq!(config.center_z, -3);
        assert_eq!(config.center_chunk_x(), 2);
        assert_eq!(config.center_chunk_z(), -3);
        assert_eq!(config.center_world_x(), 80);
        assert_eq!(config.center_world_z(), -80);
        assert_eq!(config.window().xz_scale, HEIGHTFIELD_PREVIEW_XZ_SCALE);
    }

    #[test]
    fn positional_center_defaults_to_chunk_coordinates() {
        let config = parse_args_from(["42", "2", "-3", "--chunk-radius", "1"]).expect("parse args");
        let window = config.window();

        assert!(!config.center_is_world_blocks);
        assert_eq!(config.center_chunk_x(), 2);
        assert_eq!(config.center_chunk_z(), -3);
        assert_eq!(chunk_range_for_window(window), (1, 3, -4, -2));
        assert_eq!(window.min_x(), 32.0);
        assert_eq!(window.max_x(), 128.0);
        assert_eq!(window.min_z(), -128.0);
        assert_eq!(window.max_z(), -32.0);
    }

    #[test]
    fn world_center_compat_option_derives_chunk_context() {
        let config = parse_args_from(["42", "64", "-33", "--world-center", "--chunk-radius", "2"])
            .expect("parse args");
        let window = config.window();

        assert!(config.center_is_world_blocks);
        assert_eq!(config.center_chunk_x(), 2);
        assert_eq!(config.center_chunk_z(), -2);
        assert_eq!(config.center_world_x(), 64);
        assert_eq!(config.center_world_z(), -33);
        assert_eq!(chunk_range_for_window(window), (0, 4, -4, 0));
    }

    #[test]
    fn xz_scale_options_are_not_user_facing() {
        let xz = parse_args_from(["42", "0", "0", "--xz-scale", "1"])
            .expect_err("xz-scale should be rejected");
        let horizontal = parse_args_from(["42", "0", "0", "--horizontal-subdivisions", "1"])
            .expect_err("horizontal-subdivisions should be rejected");

        assert!(xz.to_string().contains("unknown argument '--xz-scale'"));
        assert!(
            horizontal
                .to_string()
                .contains("unknown argument '--horizontal-subdivisions'")
        );
    }

    #[test]
    fn parse_block_line_toggles() {
        let default_config = parse_args_from(["42", "0", "0"]).expect("parse args");
        let disabled = parse_args_from(["42", "0", "0", "--no-block-lines"]).expect("parse args");
        let enabled = parse_args_from(["42", "0", "0", "--no-block-lines", "--block-lines"])
            .expect("parse args");

        assert!(default_config.block_lines);
        assert!(!disabled.block_lines);
        assert!(enabled.block_lines);
    }

    #[test]
    fn fixed_xz_scale_doubles_effective_columns_without_changing_footprint() {
        let config = PreviewConfig {
            seed: 42,
            center_x: 0,
            center_z: 0,
            center_is_world_blocks: false,
            width: DEFAULT_IMAGE_WIDTH,
            height: DEFAULT_IMAGE_HEIGHT,
            world_span_blocks: DEFAULT_WORLD_SPAN_BLOCKS,
            region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
            land_bias: 0.14,
            columns_x: 32,
            columns_z: Some(24),
            chunk_radius: None,
            quarter_turns: 0,
            block_lines: DEFAULT_BLOCK_LINES,
            output: None,
        }
        .window();

        assert_eq!(config.world_span_x, DEFAULT_WORLD_SPAN_BLOCKS as f32);
        assert_eq!(
            config.world_span_z,
            DEFAULT_WORLD_SPAN_BLOCKS as f32 * 24.0 / 32.0
        );
        assert_eq!(config.columns_x, 32 * HEIGHTFIELD_PREVIEW_XZ_SCALE);
        assert_eq!(config.columns_z, 24 * HEIGHTFIELD_PREVIEW_XZ_SCALE);
        assert_eq!(
            config.sample_spacing(),
            config.base_sample_spacing() / HEIGHTFIELD_PREVIEW_XZ_SCALE as f32
        );
    }

    #[test]
    fn chunk_radius_maps_to_square_chunk_footprint() {
        let config = PreviewConfig {
            seed: 42,
            center_x: 2,
            center_z: -2,
            center_is_world_blocks: false,
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
            block_lines: DEFAULT_BLOCK_LINES,
            output: None,
        };
        let window = config.window();
        let range = chunk_range_for_window(window);

        assert_eq!(config.columns_z(), DEFAULT_COLUMNS_X);
        assert_eq!(
            window.columns_x,
            DEFAULT_COLUMNS_X * HEIGHTFIELD_PREVIEW_XZ_SCALE
        );
        assert_eq!(
            window.columns_z,
            DEFAULT_COLUMNS_X * HEIGHTFIELD_PREVIEW_XZ_SCALE
        );
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
        let plan = IsoRenderPlan::new(&tile, 1280, 720, 0).expect("iso render plan");
        let span = plan.projected_height_span_px();

        assert!(span > 720.0 * 0.19);
        assert!(span < 720.0 * 0.86);
        assert!(plan.tile_w_px > 2.0);
        assert_eq!(
            plan.vertical_px_per_block, plan.tile_h_px,
            "isometric preview should render blocks with cubic x/y/z visual scale; height relief must be compressed before rendering"
        );
    }

    #[test]
    fn xz_scale_two_keeps_cubic_render_scale() {
        let scale_one = two_by_two_heightfield_tile();
        let mut scale_two = two_by_two_heightfield_tile();
        scale_two.horizontal_subdivisions = 2;
        scale_two.sample_spacing_blocks *= 0.5;
        scale_two.config.horizontal_subdivisions = 2;

        let plan_one = IsoRenderPlan::new(&scale_one, 1280, 720, 0).expect("scale one plan");
        let plan_two = IsoRenderPlan::new(&scale_two, 1280, 720, 0).expect("scale two plan");

        assert_eq!(plan_one.vertical_px_per_block, plan_one.tile_h_px);
        assert_eq!(plan_two.vertical_px_per_block, plan_two.tile_h_px);
        assert_eq!(
            plan_one.vertical_px_per_block, plan_two.vertical_px_per_block,
            "horizontal subdivisions alone must not add artificial vertical preview normalization"
        );
        assert_eq!(
            scale_one.columns[1].surface_y,
            scale_two.columns[1].surface_y
        );
    }

    #[test]
    fn cpu_iso_preview_is_nonblank() {
        let tile = two_by_two_heightfield_tile();
        let plan = IsoRenderPlan::new(&tile, 320, 180, 0).expect("iso render plan");
        let (image, stats) =
            render_heightfield_isometric(&tile, plan, DEFAULT_BLOCK_LINES).expect("render");
        let first = image.rgba.chunks_exact(4).next().expect("pixel");
        let varied = image
            .rgba
            .chunks_exact(4)
            .any(|pixel| pixel[0] != first[0] || pixel[1] != first[1] || pixel[2] != first[2]);

        assert!(varied);
        assert!(stats.projected_height_span_px > 0.0);
    }

    #[test]
    fn quarter_turns_change_visible_side_directions() {
        assert_eq!(
            visible_side_directions(0)
                .iter()
                .map(|side| (side.dx, side.dz))
                .collect::<Vec<_>>(),
            vec![(1, 0), (0, 1)]
        );
        assert_eq!(
            visible_side_directions(1)
                .iter()
                .map(|side| (side.dx, side.dz))
                .collect::<Vec<_>>(),
            vec![(-1, 0), (0, 1)]
        );
        assert_eq!(
            visible_side_directions(2)
                .iter()
                .map(|side| (side.dx, side.dz))
                .collect::<Vec<_>>(),
            vec![(-1, 0), (0, -1)]
        );
        assert_eq!(
            visible_side_directions(3)
                .iter()
                .map(|side| (side.dx, side.dz))
                .collect::<Vec<_>>(),
            vec![(1, 0), (0, -1)]
        );
    }

    #[test]
    fn projected_compass_follows_quarter_turns() {
        let n0 = iso_cardinal_screen_delta(0, 0.0, -1.0);
        let e0 = iso_cardinal_screen_delta(0, 1.0, 0.0);
        assert!(n0.x > 0.0 && n0.y < 0.0);
        assert!(e0.x > 0.0 && e0.y > 0.0);

        let n1 = iso_cardinal_screen_delta(1, 0.0, -1.0);
        let e1 = iso_cardinal_screen_delta(1, 1.0, 0.0);
        assert!(n1.x < 0.0 && n1.y < 0.0);
        assert!(e1.x > 0.0 && e1.y < 0.0);
    }

    #[test]
    fn all_quarter_turns_render_nonblank_with_block_lines() {
        let tile = two_by_two_heightfield_tile();
        for quarter in 0..4 {
            let plan = IsoRenderPlan::new(&tile, 320, 180, quarter).expect("iso render plan");
            let (image, _) =
                render_heightfield_isometric(&tile, plan, true).expect("render quarter");
            let first = image.rgba.chunks_exact(4).next().expect("pixel");
            let varied = image
                .rgba
                .chunks_exact(4)
                .any(|pixel| pixel[0] != first[0] || pixel[1] != first[1] || pixel[2] != first[2]);
            assert!(varied, "quarter {quarter} should produce visible geometry");
        }
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
            horizontal_subdivisions: 1,
            columns,
            stats: new_world::world::generation::HeightfieldTileStats {
                column_count: 4,
                min_surface_height_blocks: -8.0,
                max_surface_height_blocks: 72.0,
                average_surface_height_blocks: 27.5,
                contour_step_blocks: HeightfieldConfig::default().contour.step_blocks,
                contour_min_gap_blocks: HeightfieldConfig::default().contour.min_gap_blocks,
                contour_river_min_gap_blocks: HeightfieldConfig::default()
                    .contour
                    .river_min_gap_blocks,
                contour_band_smoothing: HeightfieldConfig::default().contour.band_smoothing,
                max_raw_neighbor_delta_blocks: 80.0,
                max_contour_guided_neighbor_delta_blocks: 80.0,
                max_constrained_neighbor_delta_blocks: 80.0,
                max_snapped_neighbor_delta_blocks: 80.0,
                max_visible_neighbor_delta_blocks: 80.0,
                max_shore_visible_neighbor_delta_blocks: 0.0,
                min_ocean_visible_surface_blocks: 0.0,
                max_ocean_visible_surface_blocks: 0.0,
                min_land_near_water_surface_blocks: 0.0,
                max_river_water_neighbor_delta_blocks: 0.0,
                river_uphill_flow_neighbor_count: 0,
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
            contour_guided_surface_height_blocks: surface_height_blocks,
            constrained_surface_height_blocks: surface_height_blocks,
            surface_height_blocks,
            surface_y: surface_height_blocks.floor() as i32,
            water_level_blocks: None,
            water_y: None,
            river_water_height_blocks: None,
            terrain_kind,
            macro_elevation: 0.0,
            combined_macro_height: surface_height_blocks / DEFAULT_HEIGHTFIELD_MAX_BLOCKS,
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
