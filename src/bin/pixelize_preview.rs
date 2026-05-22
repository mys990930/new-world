use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{self, BufWriter, ErrorKind};
use std::path::{Path, PathBuf};
use std::time::Instant;

use image::RgbImage;
use rayon::prelude::*;

use new_world::world::generation::{
    BoundaryCache, DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS, GraphRegionArea,
    MacroFieldTile, MacroFieldTileConfig, MacroMapConfig, PixelizeConfig, PixelizedChunkArea,
    PixelizedColumn, PixelizedTerrainKind, WorldPlanePoint, generate_macro_field_tile,
    generate_pixelized_chunk_area, graph_region_for_world_block,
};
use new_world::world::{CHUNK_EDGE_I32, WorldMeta};

mod common;

use common::generation_preview_context::{
    CommonPreviewWorld, PreviewStageInput, build_common_preview_world,
};
use common::preview_compass::draw_compass_rgb;

const DEFAULT_STAGE: &str = "pixelize";
const OUTPUT_DIR: &str = "target/pixelize-preview";
const GRID_COLOR: [u8; 3] = [238, 241, 232];
const LEGEND_PANEL: [u8; 3] = [8, 12, 15];
const TEXT_COLOR: [u8; 3] = [235, 240, 230];
const WATER_COLOR: [u8; 3] = [45, 115, 178];
const LAKE_COLOR: [u8; 3] = [57, 151, 196];
const RIVER_COLOR: [u8; 3] = [66, 180, 216];
const DRY_COLOR: [u8; 3] = [128, 112, 132];
const COAST_COLOR: [u8; 3] = [214, 190, 118];
const LAND_LOW_COLOR: [u8; 3] = [78, 132, 86];
const LAND_HIGH_COLOR: [u8; 3] = [220, 224, 214];
const RIDGE_COLOR: [u8; 3] = [232, 234, 224];
const LETTERBOX_COLOR: [u8; 3] = [18, 22, 24];
const BOUNDARY_EDGE_COLOR: [u8; 3] = [10, 15, 18];
const BOUNDARY_EDGE_HIGHLIGHT: [u8; 3] = [222, 232, 218];

#[derive(Debug, Clone, PartialEq)]
struct PreviewConfig {
    seed: u64,
    center_chunk_x: i32,
    center_chunk_z: i32,
    radius: i32,
    width: Option<u32>,
    height: Option<u32>,
    region_size_blocks: i32,
    site_spacing_blocks: i32,
    land_bias: f32,
    stage: String,
    output: Option<PathBuf>,
}

impl PreviewConfig {
    fn validate(self) -> Result<Self, Box<dyn Error>> {
        if self.radius < 0 {
            return Err(cli_error("r must be zero or positive"));
        }
        if self.width == Some(0) || self.height == Some(0) {
            return Err(cli_error("width and height must be positive when provided"));
        }
        if self.region_size_blocks <= 0 {
            return Err(cli_error("region-size-blocks must be positive"));
        }
        if self.site_spacing_blocks <= 0 {
            return Err(cli_error("site-spacing-blocks must be positive"));
        }
        if !self.land_bias.is_finite() {
            return Err(cli_error("land-bias must be finite"));
        }
        if self.stage != DEFAULT_STAGE {
            return Err(cli_error(format!(
                "unsupported stage: {} (expected {DEFAULT_STAGE})",
                self.stage
            )));
        }
        self.window()?;
        Ok(self)
    }

    fn window(&self) -> Result<PreviewWindow, Box<dyn Error>> {
        PreviewWindow::new(self.center_chunk_x, self.center_chunk_z, self.radius)
    }

    fn output_width(&self, window: PreviewWindow) -> u32 {
        self.width.unwrap_or(window.columns_x)
    }

    fn output_height(&self, window: PreviewWindow) -> u32 {
        self.height.unwrap_or(window.columns_z)
    }

    fn default_file_name(&self) -> String {
        format!(
            "s{}_cx{}_cz{}_r{}.png",
            self.seed, self.center_chunk_x, self.center_chunk_z, self.radius
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PreviewWindow {
    min_chunk_x: i32,
    max_chunk_x: i32,
    min_chunk_z: i32,
    max_chunk_z: i32,
    min_world_x: i32,
    max_world_x_exclusive: i32,
    min_world_z: i32,
    max_world_z_exclusive: i32,
    center_world_x: i32,
    center_world_z: i32,
    columns_x: u32,
    columns_z: u32,
    chunk_count_per_axis: u32,
}

impl PreviewWindow {
    fn new(center_chunk_x: i32, center_chunk_z: i32, radius: i32) -> Result<Self, Box<dyn Error>> {
        let min_chunk_x = center_chunk_x
            .checked_sub(radius)
            .ok_or_else(|| cli_error("minimum chunk x overflowed"))?;
        let max_chunk_x = center_chunk_x
            .checked_add(radius)
            .ok_or_else(|| cli_error("maximum chunk x overflowed"))?;
        let min_chunk_z = center_chunk_z
            .checked_sub(radius)
            .ok_or_else(|| cli_error("minimum chunk z overflowed"))?;
        let max_chunk_z = center_chunk_z
            .checked_add(radius)
            .ok_or_else(|| cli_error("maximum chunk z overflowed"))?;
        let chunk_count = radius
            .checked_mul(2)
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| cli_error("chunk radius overflowed"))?;
        let block_count = chunk_count
            .checked_mul(CHUNK_EDGE_I32)
            .ok_or_else(|| cli_error("chunk footprint overflowed"))?;
        let min_world_x = min_chunk_x
            .checked_mul(CHUNK_EDGE_I32)
            .ok_or_else(|| cli_error("minimum world x overflowed"))?;
        let min_world_z = min_chunk_z
            .checked_mul(CHUNK_EDGE_I32)
            .ok_or_else(|| cli_error("minimum world z overflowed"))?;
        let max_world_x_exclusive = max_chunk_x
            .checked_add(1)
            .and_then(|value| value.checked_mul(CHUNK_EDGE_I32))
            .ok_or_else(|| cli_error("maximum world x overflowed"))?;
        let max_world_z_exclusive = max_chunk_z
            .checked_add(1)
            .and_then(|value| value.checked_mul(CHUNK_EDGE_I32))
            .ok_or_else(|| cli_error("maximum world z overflowed"))?;
        let center_world_x = center_chunk_x
            .checked_mul(CHUNK_EDGE_I32)
            .and_then(|value| value.checked_add(CHUNK_EDGE_I32 / 2))
            .ok_or_else(|| cli_error("center world x overflowed"))?;
        let center_world_z = center_chunk_z
            .checked_mul(CHUNK_EDGE_I32)
            .and_then(|value| value.checked_add(CHUNK_EDGE_I32 / 2))
            .ok_or_else(|| cli_error("center world z overflowed"))?;

        Ok(Self {
            min_chunk_x,
            max_chunk_x,
            min_chunk_z,
            max_chunk_z,
            min_world_x,
            max_world_x_exclusive,
            min_world_z,
            max_world_z_exclusive,
            center_world_x,
            center_world_z,
            columns_x: u32::try_from(block_count)
                .map_err(|_| cli_error("column width overflowed"))?,
            columns_z: u32::try_from(block_count)
                .map_err(|_| cli_error("column height overflowed"))?,
            chunk_count_per_axis: u32::try_from(chunk_count)
                .map_err(|_| cli_error("chunk count overflowed"))?,
        })
    }

    fn graph_area(self, region_size_blocks: i32) -> Result<GraphRegionArea, Box<dyn Error>> {
        let max_world_x = self
            .max_world_x_exclusive
            .checked_sub(1)
            .ok_or_else(|| cli_error("empty world x footprint"))?;
        let max_world_z = self
            .max_world_z_exclusive
            .checked_sub(1)
            .ok_or_else(|| cli_error("empty world z footprint"))?;
        let min =
            graph_region_for_world_block(self.min_world_x, self.min_world_z, region_size_blocks);
        let max = graph_region_for_world_block(max_world_x, max_world_z, region_size_blocks);
        GraphRegionArea::new(min, max).ok_or_else(|| cli_error("invalid pixelize preview area"))
    }

    fn column_count(self) -> usize {
        self.columns_x as usize * self.columns_z as usize
    }

    fn clip_noisy_world_segment_to_chunk(
        self,
        a: WorldPlanePoint,
        b: WorldPlanePoint,
    ) -> Option<(WorldPlanePoint, WorldPlanePoint)> {
        let dx = b.x - a.x;
        let dz = b.z - a.z;
        let mut enter = 0.0;
        let mut exit = 1.0;

        if !clip_segment_axis(-dx, a.x - self.min_world_x as f32, &mut enter, &mut exit)
            || !clip_segment_axis(
                dx,
                self.max_world_x_exclusive as f32 - a.x,
                &mut enter,
                &mut exit,
            )
            || !clip_segment_axis(-dz, a.z - self.min_world_z as f32, &mut enter, &mut exit)
            || !clip_segment_axis(
                dz,
                self.max_world_z_exclusive as f32 - a.z,
                &mut enter,
                &mut exit,
            )
        {
            return None;
        }

        Some((
            WorldPlanePoint::new(a.x + dx * enter, a.z + dz * enter),
            WorldPlanePoint::new(a.x + dx * exit, a.z + dz * exit),
        ))
    }

    fn snap_world_point_to_pixelize_lattice(self, point: WorldPlanePoint) -> LatticePoint {
        let x = (point.x - self.min_world_x as f32)
            .round()
            .clamp(0.0, self.columns_x.saturating_sub(1) as f32) as i32;
        let z = (point.z - self.min_world_z as f32)
            .round()
            .clamp(0.0, self.columns_z.saturating_sub(1) as f32) as i32;
        LatticePoint { x, z }
    }

    fn project_lattice_point_to_viewport_pixel(
        self,
        viewport: MapViewport,
        point: LatticePoint,
    ) -> (i32, i32) {
        let x = viewport.x as i32
            + (point.x.max(0) as u64 * viewport.size as u64 / self.columns_x.max(1) as u64)
                .min(viewport.size.saturating_sub(1) as u64) as i32;
        let y = viewport.y as i32
            + ((self.columns_z.saturating_sub(1) as i32 - point.z.max(0)).max(0) as u64
                * viewport.size as u64
                / self.columns_z.max(1) as u64)
                .min(viewport.size.saturating_sub(1) as u64) as i32;
        (x, y)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LatticePoint {
    x: i32,
    z: i32,
}

fn clip_segment_axis(p: f32, q: f32, enter: &mut f32, exit: &mut f32) -> bool {
    if p.abs() <= f32::EPSILON {
        return q >= 0.0;
    }

    let t = q / p;
    if p < 0.0 {
        if t > *exit {
            return false;
        }
        *enter = (*enter).max(t);
    } else {
        if t < *enter {
            return false;
        }
        *exit = (*exit).min(t);
    }
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MapViewport {
    x: u32,
    y: u32,
    size: u32,
}

impl MapViewport {
    fn new(output_width: u32, output_height: u32) -> Self {
        let size = output_width.min(output_height);
        Self {
            x: (output_width - size) / 2,
            y: (output_height - size) / 2,
            size,
        }
    }

    fn contains(self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.x + self.size && y >= self.y && y < self.y + self.size
    }

    fn local_x(self, x: u32) -> u32 {
        x - self.x
    }

    fn local_y(self, y: u32) -> u32 {
        y - self.y
    }
}

#[derive(Debug, Clone)]
struct PreparedPixelizePreview {
    pixelized: PixelizedChunkArea,
    boundary: BoundaryCache,
    graph_site_count: usize,
    graph_edge_count: usize,
    macro_sample_count: usize,
    hydrology_segment_count: usize,
    boundary_curve_count: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct ColumnStats {
    min_surface_y: i32,
    max_surface_y: i32,
    average_surface_y: f32,
    water_columns: usize,
    ocean_columns: usize,
    lake_columns: usize,
    river_columns: usize,
    dry_columns: usize,
    coast_columns: usize,
    ridge_columns: usize,
}

impl ColumnStats {
    fn from_area(area: &PixelizedChunkArea) -> Self {
        if area.columns.is_empty() {
            return Self::default();
        }

        let mut min_surface_y = i32::MAX;
        let mut max_surface_y = i32::MIN;
        let mut sum_surface_y = 0_i64;
        let mut stats = Self::default();
        for column in &area.columns {
            min_surface_y = min_surface_y.min(column.surface_y);
            max_surface_y = max_surface_y.max(column.surface_y);
            sum_surface_y += i64::from(column.surface_y);
            stats.water_columns += usize::from(column.water_y.is_some());
            stats.ocean_columns += usize::from(column.terrain_kind == PixelizedTerrainKind::Ocean);
            stats.lake_columns += usize::from(column.terrain_kind == PixelizedTerrainKind::Lake);
            stats.river_columns += usize::from(is_river_column(column));
            stats.dry_columns += usize::from(column.terrain_kind == PixelizedTerrainKind::DryBasin);
            stats.coast_columns += usize::from(column.terrain_kind == PixelizedTerrainKind::Coast);
            stats.ridge_columns += usize::from(column.terrain_kind == PixelizedTerrainKind::Ridge);
        }

        stats.min_surface_y = min_surface_y;
        stats.max_surface_y = max_surface_y;
        stats.average_surface_y = sum_surface_y as f32 / area.columns.len() as f32;
        stats
    }
}

#[derive(Debug, Clone)]
struct PreviewHeader {
    seed: u64,
    generator_version: u32,
    stage: String,
    center_chunk_x: i32,
    center_chunk_z: i32,
    radius: i32,
    min_chunk_x: i32,
    max_chunk_x: i32,
    min_chunk_z: i32,
    max_chunk_z: i32,
    min_world_x: i32,
    max_world_x_exclusive: i32,
    min_world_z: i32,
    max_world_z_exclusive: i32,
    output_width: u32,
    output_height: u32,
    map_viewport_x: u32,
    map_viewport_y: u32,
    map_viewport_size: u32,
    columns_x: u32,
    columns_z: u32,
    chunk_edge_blocks: i32,
    region_size_blocks: i32,
    site_spacing_blocks: i32,
    land_bias: f32,
    graph_area: GraphRegionArea,
    graph_site_count: usize,
    graph_edge_count: usize,
    macro_sample_count: usize,
    pixelized_column_count: usize,
    hydrology_segment_count: usize,
    boundary_curve_count: usize,
    build_ms: u128,
    render_ms: u128,
    total_ms: u128,
    stats: ColumnStats,
}

impl PreviewHeader {
    fn to_metadata_text(&self) -> String {
        [
            "binary=pixelize_preview".to_string(),
            format!("seed={}", self.seed),
            format!("generator_version={}", self.generator_version),
            format!("stage={}", self.stage),
            "orientation_overlay=north_up_east_right".to_string(),
            format!(
                "center_chunk={},{}",
                self.center_chunk_x, self.center_chunk_z
            ),
            format!("chunk_radius={}", self.radius),
            format!(
                "chunk_range_xz={}..{},{}..{}",
                self.min_chunk_x, self.max_chunk_x, self.min_chunk_z, self.max_chunk_z
            ),
            format!(
                "world_footprint_blocks=x:{}..{},z:{}..{}",
                self.min_world_x,
                self.max_world_x_exclusive - 1,
                self.min_world_z,
                self.max_world_z_exclusive - 1
            ),
            format!("output_image={}x{}", self.output_width, self.output_height),
            format!(
                "map_viewport=x:{} y:{} size:{}x{}",
                self.map_viewport_x,
                self.map_viewport_y,
                self.map_viewport_size,
                self.map_viewport_size
            ),
            format!("pixelized_columns={}x{}", self.columns_x, self.columns_z),
            "column_density=one_pixelized_column_per_world_block".to_string(),
            format!("chunk_edge_blocks={}", self.chunk_edge_blocks),
            format!("region_size_blocks={}", self.region_size_blocks),
            format!("site_spacing_blocks={}", self.site_spacing_blocks),
            format!("land_bias={:.6}", self.land_bias),
            format!(
                "graph_area_min={},{}",
                self.graph_area.min.x, self.graph_area.min.z
            ),
            format!(
                "graph_area_max={},{}",
                self.graph_area.max.x, self.graph_area.max.z
            ),
            format!("graph_site_count={}", self.graph_site_count),
            format!("graph_edge_count={}", self.graph_edge_count),
            format!("macro_sample_count={}", self.macro_sample_count),
            format!("pixelized_column_count={}", self.pixelized_column_count),
            format!("hydrology_segment_count={}", self.hydrology_segment_count),
            format!("boundary_curve_count={}", self.boundary_curve_count),
            format!(
                "surface_y_min_avg_max={},{:.3},{}",
                self.stats.min_surface_y, self.stats.average_surface_y, self.stats.max_surface_y
            ),
            format!(
                "water_ocean_lake_river_dry_coast_ridge_columns={},{},{},{},{},{},{}",
                self.stats.water_columns,
                self.stats.ocean_columns,
                self.stats.lake_columns,
                self.stats.river_columns,
                self.stats.dry_columns,
                self.stats.coast_columns,
                self.stats.ridge_columns
            ),
            "render=topdown_height_color_ramp_with_chunk_grid_boundary_cache_overlay_compass_legend"
                .to_string(),
            "map_viewport=aspect_preserving_square_centered_with_letterbox".to_string(),
            "graph_overlay=boundary_cache_canonical_noisy_curves_clipped_to_chunk_footprint_snapped_to_pixelize_lattice_orthogonal_grid_path".to_string(),
            "world_api=new_world::world::generation::generate_pixelized_chunk_area".to_string(),
            "pixelize_input=graph_macro_hydrology_boundary_macro_field_tile".to_string(),
            format!(
                "timing_ms=build:{} render_encode:{} total:{}",
                self.build_ms, self.render_ms, self.total_ms
            ),
        ]
        .join("\n")
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let total_start = Instant::now();
    let config = parse_args()?.validate()?;
    let meta = WorldMeta::new(config.seed);
    let window = config.window()?;
    let graph_area = window.graph_area(config.region_size_blocks)?;

    let build_start = Instant::now();
    let prepared = build_prepared_pixelize_preview(&meta, &config, window, graph_area)?;
    let build_ms = build_start.elapsed().as_millis();

    validate_pixelized_area(&prepared.pixelized, window)?;
    let stats = ColumnStats::from_area(&prepared.pixelized);
    let output_width = config.output_width(window);
    let output_height = config.output_height(window);
    let output = output_path_for_config(&config);

    let render_start = Instant::now();
    let viewport = MapViewport::new(output_width, output_height);
    let mut image = render_pixelized_area(
        &prepared.pixelized,
        stats,
        output_width,
        output_height,
        viewport,
    )?;
    draw_chunk_grid(&mut image, window, viewport);
    draw_boundary_cache_edges(&mut image, &prepared.boundary, window, viewport);
    draw_legend(&mut image, &config, window, stats);
    draw_compass_rgb(&mut image);
    let total_ms = total_start.elapsed().as_millis();
    let header = PreviewHeader {
        seed: config.seed,
        generator_version: meta.generator_version,
        stage: config.stage.clone(),
        center_chunk_x: config.center_chunk_x,
        center_chunk_z: config.center_chunk_z,
        radius: config.radius,
        min_chunk_x: window.min_chunk_x,
        max_chunk_x: window.max_chunk_x,
        min_chunk_z: window.min_chunk_z,
        max_chunk_z: window.max_chunk_z,
        min_world_x: window.min_world_x,
        max_world_x_exclusive: window.max_world_x_exclusive,
        min_world_z: window.min_world_z,
        max_world_z_exclusive: window.max_world_z_exclusive,
        output_width,
        output_height,
        map_viewport_x: viewport.x,
        map_viewport_y: viewport.y,
        map_viewport_size: viewport.size,
        columns_x: window.columns_x,
        columns_z: window.columns_z,
        chunk_edge_blocks: CHUNK_EDGE_I32,
        region_size_blocks: config.region_size_blocks,
        site_spacing_blocks: config.site_spacing_blocks,
        land_bias: config.land_bias,
        graph_area,
        graph_site_count: prepared.graph_site_count,
        graph_edge_count: prepared.graph_edge_count,
        macro_sample_count: prepared.macro_sample_count,
        pixelized_column_count: prepared.pixelized.columns.len(),
        hydrology_segment_count: prepared.hydrology_segment_count,
        boundary_curve_count: prepared.boundary_curve_count,
        build_ms,
        render_ms: render_start.elapsed().as_millis(),
        total_ms,
        stats,
    };
    write_png_with_metadata(&image, &output, &header)?;

    println!("seed: {}", config.seed);
    println!("generator version: {}", meta.generator_version);
    println!("stage: {}", config.stage);
    println!(
        "center chunk: ({}, {}) radius {}",
        config.center_chunk_x, config.center_chunk_z, config.radius
    );
    println!(
        "chunk range x={}..{} z={}..{}",
        window.min_chunk_x, window.max_chunk_x, window.min_chunk_z, window.max_chunk_z
    );
    println!(
        "pixelized columns: {}x{} ({})",
        window.columns_x,
        window.columns_z,
        prepared.pixelized.columns.len()
    );
    println!(
        "render viewport: {}x{} square at ({}, {}) in {}x{} output",
        viewport.size, viewport.size, viewport.x, viewport.y, output_width, output_height
    );
    println!(
        "overlay: clipped BoundaryCache canonical noisy boundary curves ({} curves, {} graph edges)",
        prepared.boundary_curve_count, prepared.graph_edge_count
    );
    println!(
        "surface y min/avg/max: {} / {:.2} / {}",
        stats.min_surface_y, stats.average_surface_y, stats.max_surface_y
    );
    println!(
        "water/ocean/lake/river/dry/coast/ridge columns: {}/{}/{}/{}/{}/{}/{}",
        stats.water_columns,
        stats.ocean_columns,
        stats.lake_columns,
        stats.river_columns,
        stats.dry_columns,
        stats.coast_columns,
        stats.ridge_columns
    );
    println!("output: {}", output.display());
    println!("metadata: new-world-preview-header iTXt chunk");

    Ok(())
}

fn build_prepared_pixelize_preview(
    meta: &WorldMeta,
    config: &PreviewConfig,
    window: PreviewWindow,
    graph_area: GraphRegionArea,
) -> Result<PreparedPixelizePreview, Box<dyn Error>> {
    let inputs = build_generation_inputs(meta, config, window, graph_area)?;
    let macro_tile = build_macro_field_tile(window, &inputs);
    let pixelized = generate_pixelized_chunk_area(&macro_tile, PixelizeConfig::default());

    Ok(PreparedPixelizePreview {
        graph_site_count: inputs.patch.sites.len(),
        graph_edge_count: inputs.patch.edges.len(),
        macro_sample_count: macro_tile.samples.len(),
        hydrology_segment_count: inputs.hydrology.segments.len(),
        boundary_curve_count: inputs.boundary.curves.len(),
        boundary: inputs.boundary,
        pixelized,
    })
}

fn build_generation_inputs(
    meta: &WorldMeta,
    config: &PreviewConfig,
    window: PreviewWindow,
    graph_area: GraphRegionArea,
) -> Result<CommonPreviewWorld, Box<dyn Error>> {
    build_common_preview_world(
        meta,
        PreviewStageInput {
            center_world_x: window.center_world_x,
            center_world_z: window.center_world_z,
            region_size_blocks: config.region_size_blocks,
            site_spacing_blocks: config.site_spacing_blocks,
            land_bias: config.land_bias,
            graph_area,
        },
    )
    .map_err(cli_error)
}

fn build_macro_field_tile(window: PreviewWindow, inputs: &CommonPreviewWorld) -> MacroFieldTile {
    let sample_spacing = 1.0;
    let config = MacroFieldTileConfig::new(
        window.min_world_x as f32,
        window.min_world_z as f32,
        window.columns_x,
        window.columns_z,
        sample_spacing,
    );

    generate_macro_field_tile(
        &inputs.patch,
        &inputs.macro_map,
        &inputs.river_plan,
        &inputs.boundary,
        config,
    )
}

fn validate_pixelized_area(
    area: &PixelizedChunkArea,
    window: PreviewWindow,
) -> Result<(), Box<dyn Error>> {
    if area.columns.len() != window.column_count() {
        return Err(cli_error(format!(
            "pixelized column count mismatch: got {}, expected {}",
            area.columns.len(),
            window.column_count()
        )));
    }
    if area.width != window.columns_x || area.height != window.columns_z {
        return Err(cli_error(format!(
            "pixelized area dimensions mismatch: got {}x{}, expected {}x{}",
            area.width, area.height, window.columns_x, window.columns_z
        )));
    }
    Ok(())
}

fn render_pixelized_area(
    area: &PixelizedChunkArea,
    stats: ColumnStats,
    output_width: u32,
    output_height: u32,
    viewport: MapViewport,
) -> Result<RgbImage, Box<dyn Error>> {
    let pixel_count = output_width as usize * output_height as usize;
    let mut pixels = vec![0_u8; pixel_count * 3];
    pixels
        .par_chunks_mut(3)
        .enumerate()
        .for_each(|(index, pixel)| {
            let x = index % output_width as usize;
            let y = index / output_width as usize;
            let color = if viewport.contains(x as u32, y as u32) {
                let local_x = viewport.local_x(x as u32);
                let local_y = viewport.local_y(y as u32);
                let sx = (local_x as u64 * area.width as u64 / viewport.size as u64)
                    .min(area.width.saturating_sub(1) as u64) as u32;
                let sz = (local_y as u64 * area.height as u64 / viewport.size as u64)
                    .min(area.height.saturating_sub(1) as u64) as u32;
                let sz = area.height.saturating_sub(1).saturating_sub(sz);
                let source_index = sz as usize * area.width as usize + sx as usize;
                area.columns
                    .get(source_index)
                    .map(|column| color_for_column(column, stats))
                    .unwrap_or([0, 0, 0])
            } else {
                LETTERBOX_COLOR
            };
            pixel.copy_from_slice(&color);
        });

    RgbImage::from_raw(output_width, output_height, pixels)
        .ok_or_else(|| cli_error("failed to assemble RGB image"))
}

fn color_for_column(column: &PixelizedColumn, stats: ColumnStats) -> [u8; 3] {
    let height_t = if stats.max_surface_y > stats.min_surface_y {
        (column.surface_y - stats.min_surface_y) as f32
            / (stats.max_surface_y - stats.min_surface_y) as f32
    } else {
        0.5
    }
    .clamp(0.0, 1.0);

    let mut color = match column.terrain_kind {
        PixelizedTerrainKind::Ocean => WATER_COLOR,
        PixelizedTerrainKind::Lake => LAKE_COLOR,
        PixelizedTerrainKind::River => RIVER_COLOR,
        PixelizedTerrainKind::DryBasin => DRY_COLOR,
        PixelizedTerrainKind::Ridge => RIDGE_COLOR,
        PixelizedTerrainKind::Coast => COAST_COLOR,
        PixelizedTerrainKind::Land => mix_rgb(LAND_LOW_COLOR, LAND_HIGH_COLOR, height_t),
    };

    if !matches!(
        column.terrain_kind,
        PixelizedTerrainKind::Ocean | PixelizedTerrainKind::Lake | PixelizedTerrainKind::River
    ) {
        let shade = 0.72 + height_t * 0.28;
        color = scale_rgb(color, shade);
    }
    if column.water_y.is_some()
        && !matches!(
            column.terrain_kind,
            PixelizedTerrainKind::Ocean | PixelizedTerrainKind::Lake
        )
    {
        color = mix_rgb(color, RIVER_COLOR, 0.35);
    }
    color
}

fn is_river_column(column: &PixelizedColumn) -> bool {
    column.terrain_kind == PixelizedTerrainKind::River
}

fn draw_chunk_grid(image: &mut RgbImage, window: PreviewWindow, viewport: MapViewport) {
    let thickness = (viewport.size / 720).clamp(1, 3) as i32;
    let chunk_count = window.chunk_count_per_axis as usize;
    for chunk_offset in 0..=chunk_count {
        let x = viewport.x as i32
            + (chunk_offset as u64 * viewport.size as u64 / chunk_count.max(1) as u64)
                .min(viewport.size.saturating_sub(1) as u64) as i32;
        draw_vertical_line_segment(
            image,
            x,
            viewport.y as i32,
            (viewport.y + viewport.size).saturating_sub(1) as i32,
            GRID_COLOR,
            0.38,
            thickness,
        );
        let y = viewport.y as i32
            + (chunk_offset as u64 * viewport.size as u64 / chunk_count.max(1) as u64)
                .min(viewport.size.saturating_sub(1) as u64) as i32;
        draw_horizontal_line_segment(
            image,
            y,
            viewport.x as i32,
            (viewport.x + viewport.size).saturating_sub(1) as i32,
            GRID_COLOR,
            0.38,
            thickness,
        );
    }
}

fn draw_boundary_cache_edges(
    image: &mut RgbImage,
    boundary: &BoundaryCache,
    window: PreviewWindow,
    viewport: MapViewport,
) {
    let width = (viewport.size / 960).min(1) as i32;
    for curve in &boundary.curves {
        if !noisy_polyline_may_overlap_window(window, &curve.points) {
            continue;
        }
        emit_clipped_noisy_polyline_orthogonal_pixel_segments(
            window,
            viewport,
            &curve.points,
            |start, end| {
                draw_orthogonal_line_segment(image, start, end, BOUNDARY_EDGE_COLOR, 0.20, width);
                draw_orthogonal_line_segment(image, start, end, BOUNDARY_EDGE_HIGHLIGHT, 0.06, 0);
            },
        );
    }
}

fn noisy_polyline_may_overlap_window(window: PreviewWindow, points: &[WorldPlanePoint]) -> bool {
    if points.len() < 2 {
        return false;
    }

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for point in points {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_z = min_z.min(point.z);
        max_z = max_z.max(point.z);
    }

    max_x >= window.min_world_x as f32
        && min_x <= window.max_world_x_exclusive as f32
        && max_z >= window.min_world_z as f32
        && min_z <= window.max_world_z_exclusive as f32
}

fn emit_clipped_noisy_polyline_orthogonal_pixel_segments<F>(
    window: PreviewWindow,
    viewport: MapViewport,
    points: &[WorldPlanePoint],
    mut emit: F,
) where
    F: FnMut((i32, i32), (i32, i32)),
{
    if !noisy_polyline_may_overlap_window(window, points) {
        return;
    }

    for pair in points.windows(2) {
        let Some((start, end)) = window.clip_noisy_world_segment_to_chunk(pair[0], pair[1]) else {
            continue;
        };
        let start = window.snap_world_point_to_pixelize_lattice(start);
        let end = window.snap_world_point_to_pixelize_lattice(end);
        emit_lattice_endpoints_as_orthogonal_path(start, end, |start, end| {
            let start = window.project_lattice_point_to_viewport_pixel(viewport, start);
            let end = window.project_lattice_point_to_viewport_pixel(viewport, end);
            if start != end {
                emit(start, end);
            }
        });
    }
}

fn emit_lattice_endpoints_as_orthogonal_path<F>(start: LatticePoint, end: LatticePoint, mut emit: F)
where
    F: FnMut(LatticePoint, LatticePoint),
{
    let mut previous = start;
    let mut current = start;
    let dx = (end.x - start.x).abs();
    let dz = (end.z - start.z).abs();
    let sx = (end.x - start.x).signum();
    let sz = (end.z - start.z).signum();

    let mut emit_point = |point: LatticePoint| {
        if previous != point {
            emit(previous, point);
            previous = point;
        }
    };

    if dx >= dz {
        let mut error = 0;
        for _ in 0..dx {
            current.x += sx;
            emit_point(current);
            error += dz;
            if error >= dx && current.z != end.z {
                current.z += sz;
                emit_point(current);
                error -= dx;
            }
        }
    } else {
        let mut error = 0;
        for _ in 0..dz {
            current.z += sz;
            emit_point(current);
            error += dx;
            if error >= dz && current.x != end.x {
                current.x += sx;
                emit_point(current);
                error -= dz;
            }
        }
    }

    if current != end {
        if current.x != end.x {
            current.x = end.x;
            emit_point(current);
        }
        if current.z != end.z {
            current.z = end.z;
            emit_point(current);
        }
    }
}

#[cfg(test)]
fn clipped_noisy_polyline_orthogonal_pixel_segments(
    window: PreviewWindow,
    viewport: MapViewport,
    points: &[WorldPlanePoint],
) -> Vec<((i32, i32), (i32, i32))> {
    let mut segments = Vec::new();
    emit_clipped_noisy_polyline_orthogonal_pixel_segments(
        window,
        viewport,
        points,
        |start, end| {
            segments.push((start, end));
        },
    );
    segments
}

#[cfg(test)]
fn expand_lattice_endpoints_to_orthogonal_path(
    start: LatticePoint,
    end: LatticePoint,
) -> Vec<LatticePoint> {
    let mut path = vec![start];
    emit_lattice_endpoints_as_orthogonal_path(start, end, |_, end| {
        push_lattice_point(&mut path, end)
    });
    path
}

#[cfg(test)]
fn push_lattice_point(path: &mut Vec<LatticePoint>, point: LatticePoint) {
    if path.last().copied() != Some(point) {
        path.push(point);
    }
}

fn draw_legend(
    image: &mut RgbImage,
    config: &PreviewConfig,
    window: PreviewWindow,
    stats: ColumnStats,
) {
    if image.width() < 96 || image.height() < 72 {
        return;
    }
    let scale = (image.width().min(image.height()) / 360).clamp(1, 4);
    let margin = (8 * scale).max(8);
    let panel_width = (160 * scale).min(image.width().saturating_sub(margin * 2).max(1));
    let panel_height = (86 * scale).min(image.height().saturating_sub(margin * 2).max(1));
    let x0 = margin;
    let y0 = image.height().saturating_sub(panel_height + margin);
    fill_rect(image, x0, y0, panel_width, panel_height, LEGEND_PANEL, 0.72);

    let text_scale = scale;
    let mut y = y0 + 7 * scale;
    draw_text(image, x0 + 8 * scale, y, "PIXELIZE", TEXT_COLOR, text_scale);
    y += 10 * scale;
    draw_text(
        image,
        x0 + 8 * scale,
        y,
        &format!(
            "CX {} CZ {} R {}",
            config.center_chunk_x, config.center_chunk_z, config.radius
        ),
        TEXT_COLOR,
        text_scale,
    );
    y += 10 * scale;
    draw_text(
        image,
        x0 + 8 * scale,
        y,
        &format!("{}X{} COLS", window.columns_x, window.columns_z),
        TEXT_COLOR,
        text_scale,
    );
    y += 12 * scale;

    draw_swatch_row(image, x0 + 8 * scale, y, LAND_LOW_COLOR, "LOW", text_scale);
    draw_swatch_row(
        image,
        x0 + 56 * scale,
        y,
        LAND_HIGH_COLOR,
        "HIGH",
        text_scale,
    );
    y += 12 * scale;
    draw_swatch_row(image, x0 + 8 * scale, y, WATER_COLOR, "WATER", text_scale);
    draw_swatch_row(image, x0 + 70 * scale, y, RIVER_COLOR, "RIVER", text_scale);
    y += 12 * scale;
    draw_swatch_row(image, x0 + 8 * scale, y, COAST_COLOR, "COAST", text_scale);
    draw_swatch_row(image, x0 + 70 * scale, y, DRY_COLOR, "DRY", text_scale);
    y += 12 * scale;
    draw_text(
        image,
        x0 + 8 * scale,
        y,
        &format!(
            "Y {} {:.0} {}",
            stats.min_surface_y, stats.average_surface_y, stats.max_surface_y
        ),
        TEXT_COLOR,
        text_scale,
    );
}

fn draw_swatch_row(image: &mut RgbImage, x: u32, y: u32, color: [u8; 3], label: &str, scale: u32) {
    let swatch = (6 * scale).max(6);
    fill_rect(image, x, y, swatch, swatch, color, 0.96);
    draw_text(image, x + swatch + 4 * scale, y, label, TEXT_COLOR, scale);
}

fn output_path_for_config(config: &PreviewConfig) -> PathBuf {
    config.output.as_ref().map_or_else(
        || PathBuf::from(OUTPUT_DIR).join(config.default_file_name()),
        |path| {
            if path.extension().is_some() {
                path.clone()
            } else {
                path.join(config.default_file_name())
            }
        },
    )
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
    if args.len() < 4 {
        return Err(cli_error(usage()));
    }
    let seed = parse_required::<u64>(&mut args, "seed")?;
    let center_chunk_x = parse_required::<i32>(&mut args, "cx")?;
    let center_chunk_z = parse_required::<i32>(&mut args, "cz")?;
    let radius = parse_required::<i32>(&mut args, "r")?;
    let mut config = PreviewConfig {
        seed,
        center_chunk_x,
        center_chunk_z,
        radius,
        width: None,
        height: None,
        region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
        site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
        land_bias: MacroMapConfig::new(seed, WorldMeta::new(seed).generator_version).land_bias,
        stage: DEFAULT_STAGE.to_string(),
        output: None,
    };

    while !args.is_empty() {
        let flag = args.remove(0);
        match flag.as_str() {
            "--width" => config.width = Some(parse_required::<u32>(&mut args, "width")?),
            "--height" => config.height = Some(parse_required::<u32>(&mut args, "height")?),
            "--region-size-blocks" => {
                config.region_size_blocks = parse_required::<i32>(&mut args, "region-size-blocks")?
            }
            "--site-spacing-blocks" => {
                config.site_spacing_blocks =
                    parse_required::<i32>(&mut args, "site-spacing-blocks")?
            }
            "--land-bias" => config.land_bias = parse_required::<f32>(&mut args, "land-bias")?,
            "--stage" => config.stage = parse_required::<String>(&mut args, "stage")?,
            "--output" => {
                config.output = Some(PathBuf::from(parse_required::<String>(
                    &mut args, "output",
                )?))
            }
            _ => return Err(cli_error(format!("unknown flag: {flag}\n\n{}", usage()))),
        }
    }

    Ok(config)
}

fn parse_required<T>(args: &mut Vec<String>, name: &str) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: Error + 'static,
{
    if args.is_empty() {
        return Err(cli_error(format!("missing {name}")));
    }
    args.remove(0)
        .parse::<T>()
        .map_err(|error| Box::new(error) as Box<dyn Error>)
}

fn usage() -> &'static str {
    "usage: cargo run --bin pixelize_preview -- <seed> <cx> <cz> <r> [--width <u32>] [--height <u32>] [--region-size-blocks <i32>] [--site-spacing-blocks <i32>] [--land-bias <f32>] [--stage pixelize] [--output <path>]"
}

fn write_png_with_metadata(
    image: &RgbImage,
    output: &Path,
    header: &PreviewHeader,
) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(output)?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, image.width(), image.height());
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.add_itxt_chunk(
        "new-world-preview-header".to_string(),
        header.to_metadata_text(),
    )?;
    encoder.add_text_chunk(
        "Software".to_string(),
        "new-world pixelize_preview".to_string(),
    )?;
    let mut png_writer = encoder.write_header()?;
    png_writer.write_image_data(image.as_raw())?;
    Ok(())
}

fn draw_vertical_line_segment(
    image: &mut RgbImage,
    x: i32,
    y0: i32,
    y1: i32,
    color: [u8; 3],
    amount: f32,
    thickness: i32,
) {
    for ox in -thickness / 2..=thickness / 2 {
        for y in y0.min(y1)..=y0.max(y1) {
            blend_pixel_i32(image, x + ox, y, color, amount);
        }
    }
}

fn draw_horizontal_line_segment(
    image: &mut RgbImage,
    y: i32,
    x0: i32,
    x1: i32,
    color: [u8; 3],
    amount: f32,
    thickness: i32,
) {
    for oy in -thickness / 2..=thickness / 2 {
        for x in x0.min(x1)..=x0.max(x1) {
            blend_pixel_i32(image, x, y + oy, color, amount);
        }
    }
}

fn draw_orthogonal_line_segment(
    image: &mut RgbImage,
    start: (i32, i32),
    end: (i32, i32),
    color: [u8; 3],
    amount: f32,
    thickness: i32,
) {
    if start.0 == end.0 {
        draw_vertical_line_segment(image, start.0, start.1, end.1, color, amount, thickness);
    } else if start.1 == end.1 {
        draw_horizontal_line_segment(image, start.1, start.0, end.0, color, amount, thickness);
    }
}

fn fill_rect(
    image: &mut RgbImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: [u8; 3],
    amount: f32,
) {
    for py in y..(y + height).min(image.height()) {
        for px in x..(x + width).min(image.width()) {
            blend_pixel(image, px, py, color, amount);
        }
    }
}

fn draw_text(image: &mut RgbImage, x: u32, y: u32, text: &str, color: [u8; 3], scale: u32) {
    let scale = scale.max(1);
    let mut cursor = x;
    for ch in text.chars() {
        let glyph = glyph_3x5(ch);
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..3_u32 {
                if (bits >> (2 - col)) & 1 == 0 {
                    continue;
                }
                for sy in 0..scale {
                    for sx in 0..scale {
                        blend_pixel(
                            image,
                            cursor + col * scale + sx,
                            y + row as u32 * scale + sy,
                            color,
                            0.96,
                        );
                    }
                }
            }
        }
        cursor += 4 * scale;
    }
}

fn glyph_3x5(ch: char) -> [u8; 5] {
    match ch.to_ascii_uppercase() {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b111, 0b100, 0b100, 0b100, 0b111],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b111, 0b100, 0b101, 0b101, 0b111],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b111],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b101, 0b111, 0b111, 0b111, 0b101],
        'O' => [0b111, 0b101, 0b101, 0b101, 0b111],
        'P' => [0b111, 0b101, 0b111, 0b100, 0b100],
        'Q' => [0b111, 0b101, 0b101, 0b111, 0b001],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b111, 0b100, 0b111, 0b001, 0b111],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        ':' => [0b000, 0b010, 0b000, 0b010, 0b000],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        ' ' => [0, 0, 0, 0, 0],
        _ => [0, 0, 0, 0, 0],
    }
}

fn blend_pixel_i32(image: &mut RgbImage, x: i32, y: i32, color: [u8; 3], amount: f32) {
    if x < 0 || y < 0 {
        return;
    }
    blend_pixel(image, x as u32, y as u32, color, amount);
}

fn blend_pixel(image: &mut RgbImage, x: u32, y: u32, color: [u8; 3], amount: f32) {
    if x >= image.width() || y >= image.height() {
        return;
    }
    let amount = amount.clamp(0.0, 1.0);
    let index = ((y as usize * image.width() as usize) + x as usize) * 3;
    let pixels: &mut [u8] = image.as_mut();
    let base = [pixels[index], pixels[index + 1], pixels[index + 2]];
    pixels[index..index + 3].copy_from_slice(&mix_rgb(base, color, amount));
}

fn mix_rgb(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        mix_channel(a[0], b[0], t),
        mix_channel(a[1], b[1], t),
        mix_channel(a[2], b[2], t),
    ]
}

fn scale_rgb(color: [u8; 3], scale: f32) -> [u8; 3] {
    [
        (color[0] as f32 * scale).round().clamp(0.0, 255.0) as u8,
        (color[1] as f32 * scale).round().clamp(0.0, 255.0) as u8,
        (color[2] as f32 * scale).round().clamp(0.0, 255.0) as u8,
    ]
}

fn mix_channel(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 * (1.0 - t) + b as f32 * t)
        .round()
        .clamp(0.0, 255.0) as u8
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_positional_chunk_contract() {
        let config = parse_args_from(["42", "2", "-3", "1"]).expect("parse args");
        let window = config.window().expect("window");

        assert_eq!(config.seed, 42);
        assert_eq!(config.center_chunk_x, 2);
        assert_eq!(config.center_chunk_z, -3);
        assert_eq!(config.radius, 1);
        assert_eq!(window.min_chunk_x, 1);
        assert_eq!(window.max_chunk_x, 3);
        assert_eq!(window.min_chunk_z, -4);
        assert_eq!(window.max_chunk_z, -2);
        assert_eq!(window.columns_x, 96);
        assert_eq!(window.columns_z, 96);
    }

    #[test]
    fn default_output_dimensions_are_one_pixel_per_world_block() {
        let config = parse_args_from(["42", "0", "0", "0"]).expect("parse args");
        let window = config.window().expect("window");

        assert_eq!(window.columns_x, CHUNK_EDGE_I32 as u32);
        assert_eq!(window.columns_z, CHUNK_EDGE_I32 as u32);
        assert_eq!(config.output_width(window), window.columns_x);
        assert_eq!(config.output_height(window), window.columns_z);
    }

    #[test]
    fn explicit_output_dimensions_are_render_scale_only() {
        let config = parse_args_from(["42", "0", "0", "1", "--width", "192", "--height", "96"])
            .expect("parse args");
        let window = config.window().expect("window");

        assert_eq!(window.columns_x, 96);
        assert_eq!(window.columns_z, 96);
        assert_eq!(config.output_width(window), 192);
        assert_eq!(config.output_height(window), 96);
    }

    #[test]
    fn landscape_output_uses_centered_square_viewport() {
        let viewport = MapViewport::new(192, 96);

        assert_eq!(
            viewport,
            MapViewport {
                x: 48,
                y: 0,
                size: 96,
            }
        );
        assert!(!viewport.contains(47, 20));
        assert!(viewport.contains(48, 20));
        assert!(viewport.contains(143, 95));
        assert!(!viewport.contains(144, 20));
    }

    #[test]
    fn portrait_output_uses_centered_square_viewport() {
        let viewport = MapViewport::new(80, 120);

        assert_eq!(
            viewport,
            MapViewport {
                x: 0,
                y: 20,
                size: 80,
            }
        );
        assert!(!viewport.contains(40, 19));
        assert!(viewport.contains(40, 20));
        assert!(viewport.contains(79, 99));
        assert!(!viewport.contains(40, 100));
    }

    #[test]
    fn world_segments_clip_to_chunk_footprint_before_viewport_projection() {
        let window = PreviewWindow::new(0, 0, 0).expect("window");
        let viewport = MapViewport::new(64, 32);

        let (start, end) = window
            .clip_noisy_world_segment_to_chunk(
                WorldPlanePoint::new(-10.0, 16.0),
                WorldPlanePoint::new(42.0, 16.0),
            )
            .expect("clipped segment");
        let start = window.project_lattice_point_to_viewport_pixel(
            viewport,
            window.snap_world_point_to_pixelize_lattice(start),
        );
        let end = window.project_lattice_point_to_viewport_pixel(
            viewport,
            window.snap_world_point_to_pixelize_lattice(end),
        );

        assert_eq!(start, (16, 15));
        assert_eq!(end, (47, 15));
        assert!(
            window
                .clip_noisy_world_segment_to_chunk(
                    WorldPlanePoint::new(-10.0, -4.0),
                    WorldPlanePoint::new(-2.0, -4.0),
                )
                .is_none()
        );
    }

    #[test]
    fn diagonal_noisy_world_segment_becomes_stair_step_overlay_path() {
        let window = PreviewWindow::new(0, 0, 0).expect("window");
        let viewport = MapViewport::new(64, 32);
        let points = [
            WorldPlanePoint::new(0.0, 0.0),
            WorldPlanePoint::new(7.8, 7.8),
        ];

        let segments = clipped_noisy_polyline_orthogonal_pixel_segments(window, viewport, &points);

        assert_eq!(segments.first().copied(), Some(((16, 31), (17, 31))));
        assert_eq!(segments.last().copied(), Some(((24, 24), (24, 23))));
        assert!(segments.len() > 2);
        assert_no_diagonal_pixel_segments(&segments);
        assert!(segments.iter().any(|(start, end)| start.0 != end.0));
        assert!(segments.iter().any(|(start, end)| start.1 != end.1));
    }

    #[test]
    fn snapped_boundary_path_coordinates_are_on_scaled_pixelize_lattice() {
        let window = PreviewWindow::new(0, 0, 1).expect("window");
        let viewport = MapViewport::new(192, 192);
        let start = window.snap_world_point_to_pixelize_lattice(WorldPlanePoint::new(-30.6, -29.8));
        let end = window.snap_world_point_to_pixelize_lattice(WorldPlanePoint::new(-27.4, -29.8));
        let path = expand_lattice_endpoints_to_orthogonal_path(start, end);
        let pixels = path
            .iter()
            .map(|point| window.project_lattice_point_to_viewport_pixel(viewport, *point))
            .collect::<Vec<_>>();

        assert_eq!(start, LatticePoint { x: 1, z: 2 });
        assert_eq!(end, LatticePoint { x: 5, z: 2 });
        assert_eq!(
            pixels,
            vec![(2, 186), (4, 186), (6, 186), (8, 186), (10, 186)]
        );
    }

    #[test]
    fn noisy_polyline_segments_clip_snap_and_keep_only_orthogonal_segments() {
        let window = PreviewWindow::new(0, 0, 0).expect("window");
        let viewport = MapViewport::new(64, 32);
        let points = [
            WorldPlanePoint::new(-4.0, 8.0),
            WorldPlanePoint::new(8.0, 8.0),
            WorldPlanePoint::new(16.4, 16.4),
            WorldPlanePoint::new(40.0, 16.4),
        ];

        let segments = clipped_noisy_polyline_orthogonal_pixel_segments(window, viewport, &points);

        assert_eq!(segments.first().copied(), Some(((16, 23), (17, 23))));
        assert_eq!(segments.last().copied(), Some(((46, 15), (47, 15))));
        assert_no_diagonal_pixel_segments(&segments);
        assert!(segments.iter().all(|(start, end)| start != end));
    }

    #[test]
    fn direct_boundary_segment_emitter_keeps_only_nonzero_orthogonal_segments() {
        let window = PreviewWindow::new(0, 0, 0).expect("window");
        let viewport = MapViewport::new(96, 32);
        let points = [
            WorldPlanePoint::new(2.0, 2.0),
            WorldPlanePoint::new(9.4, 5.7),
            WorldPlanePoint::new(20.0, 5.7),
        ];
        let mut segments = Vec::new();

        emit_clipped_noisy_polyline_orthogonal_pixel_segments(
            window,
            viewport,
            &points,
            |start, end| segments.push((start, end)),
        );

        assert!(!segments.is_empty());
        assert_no_diagonal_pixel_segments(&segments);
        assert!(segments.iter().all(|(start, end)| start != end));
    }

    fn assert_no_diagonal_pixel_segments(segments: &[((i32, i32), (i32, i32))]) {
        assert!(
            segments.iter().all(|(start, end)| {
                let dx = end.0 - start.0;
                let dy = end.1 - start.1;
                (dx == 0) ^ (dy == 0)
            }),
            "expected only 4-connected orthogonal segments, got {segments:?}"
        );
    }
}
