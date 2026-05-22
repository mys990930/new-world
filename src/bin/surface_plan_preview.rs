use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::Instant;

use image::RgbaImage;
use new_world::world::generation::{
    BoundaryCache, DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS,
    GraphGenerationStage, GraphRegionArea, HeightfieldConfig, HeightfieldPerlinConfig,
    HeightfieldTile, MacroFieldTile, MacroFieldTileConfig, MacroMapConfig, SurfaceColumnPlan,
    SurfacePlanArea, SurfacePlanConfig, WorldPlanePoint, generate_heightfield_tile,
    generate_macro_field_tile, generate_surface_plan_area, graph_region_for_world_block,
};
use new_world::world::{CHUNK_EDGE_I32, WorldMeta};

mod common;

use common::generation_preview_context::{PreviewStageInput, build_common_preview_world};

const DEFAULT_IMAGE_WIDTH: u32 = 1280;
const DEFAULT_IMAGE_HEIGHT: u32 = 720;
const DEFAULT_CHUNK_RADIUS: i32 = 0;
const WATER_ALPHA: u8 = 108;
const ISO_TILE_HEIGHT_RATIO: f32 = 0.50;
const PLAYER_CUBE_WIDTH_BLOCKS: f32 = 1.0;
const PLAYER_CUBE_DEPTH_BLOCKS: f32 = 1.0;
const PLAYER_CUBE_HEIGHT_BLOCKS: f32 = 4.0;
const PLAYER_CUBE_TOP_COLOR: [u8; 4] = [96, 255, 68, 242];
const PLAYER_CUBE_SIDE_A_COLOR: [u8; 4] = [38, 238, 112, 232];
const PLAYER_CUBE_SIDE_B_COLOR: [u8; 4] = [18, 206, 236, 226];
const NOISY_BOUNDARY_BACKING: [u8; 4] = [0, 16, 22, 82];
const NOISY_BOUNDARY_CYAN: [u8; 4] = [0, 220, 255, 150];
const NOISY_BOUNDARY_SURFACE_LIFT_BLOCKS: f32 = 0.25;

#[derive(Debug, Clone)]
struct PreviewConfig {
    seed: u64,
    center_chunk_x: i32,
    center_chunk_z: i32,
    chunk_radius: i32,
    width: u32,
    height: u32,
    region_size_blocks: i32,
    site_spacing_blocks: i32,
    land_bias: f32,
    quarter_turns: u8,
    perlin: bool,
    output: Option<PathBuf>,
}

impl PreviewConfig {
    fn validate(self) -> Result<Self, Box<dyn Error>> {
        if self.width == 0 || self.height == 0 {
            return Err(cli_error("width and height must be positive"));
        }
        if self.chunk_radius < 0 {
            return Err(cli_error("chunk-radius must be zero or positive"));
        }
        if self.region_size_blocks <= 0 || self.site_spacing_blocks <= 0 {
            return Err(cli_error("region and site spacing must be positive"));
        }
        Ok(self)
    }

    fn output_path(&self) -> PathBuf {
        self.output.clone().unwrap_or_else(|| {
            PathBuf::from(format!(
                "target/surface-plan-preview/s{}_cx{}_cz{}_q{}_r{}.png",
                self.seed,
                self.center_chunk_x,
                self.center_chunk_z,
                self.quarter_turns % 4,
                self.chunk_radius
            ))
        })
    }

    fn window(&self) -> PreviewWindow {
        let min_chunk_x = self.center_chunk_x - self.chunk_radius;
        let max_chunk_x = self.center_chunk_x + self.chunk_radius;
        let min_chunk_z = self.center_chunk_z - self.chunk_radius;
        let max_chunk_z = self.center_chunk_z + self.chunk_radius;
        let min_world_x = min_chunk_x * CHUNK_EDGE_I32;
        let min_world_z = min_chunk_z * CHUNK_EDGE_I32;
        let chunk_count = self.chunk_radius.saturating_mul(2).saturating_add(1);
        let columns = (chunk_count * CHUNK_EDGE_I32) as u32;
        PreviewWindow {
            min_world_x,
            min_world_z,
            columns_x: columns,
            columns_z: columns,
            sample_spacing_blocks: 1.0,
            min_chunk_x,
            max_chunk_x,
            min_chunk_z,
            max_chunk_z,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PreviewWindow {
    min_world_x: i32,
    min_world_z: i32,
    columns_x: u32,
    columns_z: u32,
    sample_spacing_blocks: f32,
    min_chunk_x: i32,
    max_chunk_x: i32,
    min_chunk_z: i32,
    max_chunk_z: i32,
}

impl PreviewWindow {
    fn min_x(self) -> f32 {
        self.min_world_x as f32
    }

    fn max_x(self) -> f32 {
        self.max_world_x_exclusive() as f32
    }

    fn min_z(self) -> f32 {
        self.min_world_z as f32
    }

    fn max_z(self) -> f32 {
        self.max_world_z_exclusive() as f32
    }

    fn center_world_x(self) -> i32 {
        self.min_world_x + self.columns_x as i32 / 2
    }

    fn center_world_z(self) -> i32 {
        self.min_world_z + self.columns_z as i32 / 2
    }

    fn max_world_x_exclusive(self) -> i32 {
        self.min_world_x + self.columns_x as i32
    }

    fn max_world_z_exclusive(self) -> i32 {
        self.min_world_z + self.columns_z as i32
    }

    fn graph_area(self, region_size_blocks: i32) -> Result<GraphRegionArea, Box<dyn Error>> {
        let min =
            graph_region_for_world_block(self.min_world_x, self.min_world_z, region_size_blocks);
        let max = graph_region_for_world_block(
            self.max_world_x_exclusive() - 1,
            self.max_world_z_exclusive() - 1,
            region_size_blocks,
        );
        GraphRegionArea::new(min, max).ok_or_else(|| cli_error("invalid surface plan preview area"))
    }
}

#[derive(Debug, Clone)]
struct PreviewHeader {
    seed: u64,
    generator_version: u32,
    stage: GraphGenerationStage,
    width: u32,
    height: u32,
    center_chunk_x: i32,
    center_chunk_z: i32,
    chunk_radius: i32,
    chunk_min_x: i32,
    chunk_max_x: i32,
    chunk_min_z: i32,
    chunk_max_z: i32,
    world_min_x: i32,
    world_max_x_exclusive: i32,
    world_min_z: i32,
    world_max_z_exclusive: i32,
    columns_x: u32,
    columns_z: u32,
    column_count: usize,
    water_count: usize,
    hydrology_role_counts: Vec<(String, usize)>,
    top_blocks: Vec<(String, usize)>,
    min_surface_y: i32,
    max_surface_y: i32,
    quarter_turns: u8,
    perlin_enabled: bool,
    perlin_amplitude_blocks: f32,
    perlin_max_abs_blocks: f32,
    boundary_curve_count: usize,
    boundary_point_count: usize,
    player_cube_center_x: f32,
    player_cube_center_z: f32,
    player_cube_bottom_y: f32,
    player_cube_top_y: f32,
    player_cube_sampled_columns: usize,
    build_ms: u128,
    macro_field_ms: u128,
    heightfield_ms: u128,
    surface_plan_ms: u128,
    render_ms: u128,
    total_ms: u128,
}

impl PreviewHeader {
    fn to_metadata_text(&self) -> String {
        [
            "stage=surface_plan".to_string(),
            format!("seed={}", self.seed),
            format!("generator_version={}", self.generator_version),
            format!("graph_stage={:?}", self.stage),
            format!("image={}x{}", self.width, self.height),
            format!(
                "center_chunk={},{}",
                self.center_chunk_x, self.center_chunk_z
            ),
            format!("chunk_radius={}", self.chunk_radius),
            format!(
                "chunk_range_xz={}..{},{}..{}",
                self.chunk_min_x, self.chunk_max_x, self.chunk_min_z, self.chunk_max_z
            ),
            format!(
                "world_footprint_blocks=x:{}..{},z:{}..{}",
                self.world_min_x,
                self.world_max_x_exclusive,
                self.world_min_z,
                self.world_max_z_exclusive
            ),
            format!("columns={}x{}", self.columns_x, self.columns_z),
            format!("surface_plan_columns={}", self.column_count),
            format!("water_columns={}", self.water_count),
            format!(
                "surface_y_min_max={},{}",
                self.min_surface_y, self.max_surface_y
            ),
            format!("quarter_turns={}", self.quarter_turns),
            if self.perlin_enabled {
                format!(
                    "micro_relief_blocks=perlin_enabled_amplitude:{:.2}_max_abs:{:.2}",
                    self.perlin_amplitude_blocks, self.perlin_max_abs_blocks
                )
            } else {
                "micro_relief_blocks=0_perlin_disabled".to_string()
            },
            "geometry_source=heightfield_tile_surface_y_water_y".to_string(),
            "surface_plan_policy=material_interpretation_only_heightfield_geometry_preserved"
                .to_string(),
            format!("top_blocks={}", format_counts(&self.top_blocks)),
            format!(
                "hydrology_roles={}",
                format_counts(&self.hydrology_role_counts)
            ),
            "view=cpu_isometric_surface_plan_columns".to_string(),
            "projection=mirrors_heightfield_preview_quarter_painter_order".to_string(),
            "water_policy=translucent_overlay_from_surface_column_plan_water_y".to_string(),
            "boundary_overlay=cyan_boundary_cache_noisy_curves_draped_visible_surface".to_string(),
            format!(
                "boundary_overlay_counts=curves:{},points:{}",
                self.boundary_curve_count, self.boundary_point_count
            ),
            format!(
                "player_diagnostic_cube=enabled,width:{:.1}b,depth:{:.1}b,height:{:.1}b,center_world:{:.2},{:.2},bottom_y:{:.2},top_y:{:.2},sampled_columns:{}",
                PLAYER_CUBE_WIDTH_BLOCKS,
                PLAYER_CUBE_DEPTH_BLOCKS,
                PLAYER_CUBE_HEIGHT_BLOCKS,
                self.player_cube_center_x,
                self.player_cube_center_z,
                self.player_cube_bottom_y,
                self.player_cube_top_y,
                self.player_cube_sampled_columns
            ),
            "player_diagnostic_cube_meaning=surface_plan_preview_scale_diagnostic_not_gameplay_entity".to_string(),
            format!(
                "timing_ms=build:{} macro_field:{} heightfield:{} surface_plan:{} render:{} total:{}",
                self.build_ms,
                self.macro_field_ms,
                self.heightfield_ms,
                self.surface_plan_ms,
                self.render_ms,
                self.total_ms
            ),
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
    let preview_world = build_common_preview_world(
        &meta,
        PreviewStageInput {
            center_world_x: window.center_world_x(),
            center_world_z: window.center_world_z(),
            region_size_blocks: config.region_size_blocks,
            site_spacing_blocks: config.site_spacing_blocks,
            land_bias: config.land_bias,
            graph_area,
        },
    )
    .map_err(cli_error)?;
    let build_ms = build_start.elapsed().as_millis();

    let macro_start = Instant::now();
    let macro_tile = build_macro_field_tile(window, &preview_world);
    let macro_field_ms = macro_start.elapsed().as_millis();

    let heightfield_start = Instant::now();
    let heightfield_config = preview_heightfield_config(config.perlin, &meta);
    let heightfield = generate_heightfield_tile(&macro_tile, heightfield_config);
    let heightfield_ms = heightfield_start.elapsed().as_millis();

    let surface_plan_start = Instant::now();
    let surface_config = SurfacePlanConfig::new(meta.seed, meta.generator_version);
    let surface_plan = generate_surface_plan_area(&heightfield, Some(&macro_tile), surface_config);
    let surface_plan_ms = surface_plan_start.elapsed().as_millis();
    validate_plan_shape(&surface_plan, &heightfield)?;

    let render_start = Instant::now();
    let plan = IsoRenderPlan::new(
        &heightfield,
        config.width,
        config.height,
        config.quarter_turns,
    )?;
    let player_cube = PlayerDiagnosticCube::for_area(&surface_plan, &heightfield, window)?;
    let mut image = render_surface_plan_isometric(&surface_plan, &heightfield, plan, player_cube)?;
    draw_noisy_boundary_overlay(
        &mut image,
        &preview_world.boundary,
        &surface_plan,
        &heightfield,
        plan,
        window,
    );
    let render_ms = render_start.elapsed().as_millis();

    let summary = summarize_surface_plan(&surface_plan);
    let total_ms = total_start.elapsed().as_millis();
    let header = PreviewHeader {
        seed: config.seed,
        generator_version: meta.generator_version,
        stage: GraphGenerationStage::SurfacePlan,
        width: config.width,
        height: config.height,
        center_chunk_x: config.center_chunk_x,
        center_chunk_z: config.center_chunk_z,
        chunk_radius: config.chunk_radius,
        chunk_min_x: window.min_chunk_x,
        chunk_max_x: window.max_chunk_x,
        chunk_min_z: window.min_chunk_z,
        chunk_max_z: window.max_chunk_z,
        world_min_x: window.min_world_x,
        world_max_x_exclusive: window.max_world_x_exclusive(),
        world_min_z: window.min_world_z,
        world_max_z_exclusive: window.max_world_z_exclusive(),
        columns_x: window.columns_x,
        columns_z: window.columns_z,
        column_count: surface_plan.columns.len(),
        water_count: summary.water_count,
        hydrology_role_counts: summary.hydrology_role_counts,
        top_blocks: summary.top_blocks,
        min_surface_y: summary.min_surface_y,
        max_surface_y: summary.max_surface_y,
        quarter_turns: config.quarter_turns % 4,
        perlin_enabled: heightfield.config.perlin.enabled,
        perlin_amplitude_blocks: heightfield.config.perlin.amplitude_blocks,
        perlin_max_abs_blocks: heightfield.config.perlin.max_abs_blocks,
        boundary_curve_count: preview_world.boundary.curves.len(),
        boundary_point_count: preview_world
            .boundary
            .curves
            .iter()
            .map(|curve| curve.points.len())
            .sum(),
        player_cube_center_x: player_cube.center_world_x,
        player_cube_center_z: player_cube.center_world_z,
        player_cube_bottom_y: player_cube.bottom_y,
        player_cube_top_y: player_cube.top_y(),
        player_cube_sampled_columns: player_cube.sampled_columns,
        build_ms,
        macro_field_ms,
        heightfield_ms,
        surface_plan_ms,
        render_ms,
        total_ms,
    };

    write_rgba_png_with_metadata(&image, &output, &header)?;

    println!("surface plan preview: seed {}", config.seed);
    println!("stage: {:?}", GraphGenerationStage::SurfacePlan);
    println!(
        "chunk range: cx {}..{}, cz {}..{} (radius {})",
        window.min_chunk_x,
        window.max_chunk_x,
        window.min_chunk_z,
        window.max_chunk_z,
        config.chunk_radius
    );
    println!(
        "world footprint: x {}..{}, z {}..{} blocks",
        window.min_world_x,
        window.max_world_x_exclusive(),
        window.min_world_z,
        window.max_world_z_exclusive()
    );
    println!(
        "columns: {}x{} = {}, water {}",
        window.columns_x,
        window.columns_z,
        surface_plan.columns.len(),
        header.water_count
    );
    println!(
        "hydrology roles: {}",
        format_counts(&header.hydrology_role_counts)
    );
    println!("top blocks: {}", format_counts(&header.top_blocks));
    println!(
        "surface y min/max: {}/{}",
        header.min_surface_y, header.max_surface_y
    );
    println!(
        "perlin micro relief: {}",
        if heightfield.config.perlin.enabled {
            "on"
        } else {
            "off"
        }
    );
    println!(
        "geometry source: heightfield surface/water columns; surface plan changes material interpretation only"
    );
    println!(
        "boundary overlay: cyan noisy BoundaryCache curves {}, points {}, clipped to world footprint and draped over visible surface",
        header.boundary_curve_count, header.boundary_point_count
    );
    println!(
        "player diagnostic cube: center world ({:.2}, {:.2}), size {:.0}x{:.0}x{:.0} blocks, bottom y {:.2}, top y {:.2}, sampled columns {}",
        player_cube.center_world_x,
        player_cube.center_world_z,
        PLAYER_CUBE_WIDTH_BLOCKS,
        PLAYER_CUBE_DEPTH_BLOCKS,
        PLAYER_CUBE_HEIGHT_BLOCKS,
        player_cube.bottom_y,
        player_cube.top_y(),
        player_cube.sampled_columns
    );
    println!(
        "timing: build {} ms, macro field {} ms, heightfield {} ms, surface plan {} ms, render {} ms, total {} ms",
        build_ms, macro_field_ms, heightfield_ms, surface_plan_ms, render_ms, total_ms
    );
    println!("output: {}", output.display());
    println!("metadata: new-world-preview-header iTXt chunk");

    Ok(())
}

fn build_macro_field_tile(
    window: PreviewWindow,
    world: &common::generation_preview_context::CommonPreviewWorld,
) -> MacroFieldTile {
    let config = MacroFieldTileConfig::new(
        window.min_world_x as f32,
        window.min_world_z as f32,
        window.columns_x,
        window.columns_z,
        window.sample_spacing_blocks,
    );
    generate_macro_field_tile(
        &world.patch,
        &world.macro_map,
        &world.river_plan,
        &world.boundary,
        config,
    )
}

fn preview_heightfield_config(perlin: bool, meta: &WorldMeta) -> HeightfieldConfig {
    HeightfieldConfig {
        perlin: if perlin {
            HeightfieldPerlinConfig::preview_enabled(meta.seed, meta.generator_version)
        } else {
            HeightfieldPerlinConfig::disabled(meta.seed, meta.generator_version)
        },
        ..HeightfieldConfig::default()
    }
}

fn validate_plan_shape(
    surface_plan: &SurfacePlanArea,
    heightfield: &HeightfieldTile,
) -> Result<(), Box<dyn Error>> {
    if surface_plan.columns.len() != heightfield.columns.len() {
        return Err(cli_error(format!(
            "surface plan column count {} did not match heightfield column count {}",
            surface_plan.columns.len(),
            heightfield.columns.len()
        )));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct SurfaceSummary {
    water_count: usize,
    hydrology_role_counts: Vec<(String, usize)>,
    top_blocks: Vec<(String, usize)>,
    min_surface_y: i32,
    max_surface_y: i32,
}

fn summarize_surface_plan(area: &SurfacePlanArea) -> SurfaceSummary {
    let mut water_count = 0;
    let mut role_counts = HashMap::<String, usize>::new();
    let mut top_counts = HashMap::<String, usize>::new();
    let mut min_surface_y = i32::MAX;
    let mut max_surface_y = i32::MIN;

    for column in &area.columns {
        if column.water_y.is_some() {
            water_count += 1;
        }
        *role_counts
            .entry(format!("{:?}", column.hydrology_role))
            .or_default() += 1;
        *top_counts
            .entry(format!("{:?}", column.top_block))
            .or_default() += 1;
        min_surface_y = min_surface_y.min(column.surface_y);
        max_surface_y = max_surface_y.max(column.surface_y);
    }

    SurfaceSummary {
        water_count,
        hydrology_role_counts: top_n_counts(role_counts, 12),
        top_blocks: top_n_counts(top_counts, 12),
        min_surface_y: if min_surface_y == i32::MAX {
            0
        } else {
            min_surface_y
        },
        max_surface_y: if max_surface_y == i32::MIN {
            0
        } else {
            max_surface_y
        },
    }
}

fn top_n_counts(mut counts: HashMap<String, usize>, limit: usize) -> Vec<(String, usize)> {
    let mut counts = counts.drain().collect::<Vec<_>>();
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    counts.truncate(limit);
    counts
}

fn format_counts(counts: &[(String, usize)]) -> String {
    if counts.is_empty() {
        return "none".to_string();
    }
    counts
        .iter()
        .map(|(key, value)| format!("{key}:{value}"))
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Debug, Clone, Copy)]
struct PlayerDiagnosticCube {
    center_world_x: f32,
    center_world_z: f32,
    min_grid_x: f32,
    max_grid_x: f32,
    min_grid_z: f32,
    max_grid_z: f32,
    bottom_y: f32,
    height_blocks: f32,
    sampled_columns: usize,
}

impl PlayerDiagnosticCube {
    fn for_area(
        surface_plan: &SurfacePlanArea,
        heightfield: &HeightfieldTile,
        window: PreviewWindow,
    ) -> Result<Self, Box<dyn Error>> {
        if surface_plan.columns.is_empty() {
            return Err(cli_error(
                "player diagnostic cube needs a non-empty surface plan",
            ));
        }
        let center_world_x = (window.min_x() + window.max_x()) * 0.5;
        let center_world_z = (window.min_z() + window.max_z()) * 0.5;
        let half_x = PLAYER_CUBE_WIDTH_BLOCKS * 0.5;
        let half_z = PLAYER_CUBE_DEPTH_BLOCKS * 0.5;
        let sample_spacing = window.sample_spacing_blocks.max(f32::EPSILON);
        let min_grid_x = ((center_world_x - half_x - window.min_x()) / sample_spacing)
            .clamp(0.0, heightfield.width as f32);
        let max_grid_x = ((center_world_x + half_x - window.min_x()) / sample_spacing)
            .clamp(0.0, heightfield.width as f32);
        let min_grid_z = ((center_world_z - half_z - window.min_z()) / sample_spacing)
            .clamp(0.0, heightfield.height as f32);
        let max_grid_z = ((center_world_z + half_z - window.min_z()) / sample_spacing)
            .clamp(0.0, heightfield.height as f32);

        let mut sampled_columns = 0usize;
        let mut bottom_y = f32::NEG_INFINITY;
        for (index, column) in surface_plan.columns.iter().enumerate() {
            let x = index % heightfield.width as usize;
            let z = index / heightfield.width as usize;
            let column_center_world_x = window.min_x() + (x as f32 + 0.5) * sample_spacing;
            let column_center_world_z = window.min_z() + (z as f32 + 0.5) * sample_spacing;
            if column_center_world_x >= center_world_x - half_x
                && column_center_world_x <= center_world_x + half_x
                && column_center_world_z >= center_world_z - half_z
                && column_center_world_z <= center_world_z + half_z
            {
                sampled_columns += 1;
                bottom_y = bottom_y.max(visible_surface_plan_column_height(column));
            }
        }
        if sampled_columns == 0 {
            let nearest_x = ((center_world_x - window.min_x()) / sample_spacing - 0.5)
                .round()
                .clamp(0.0, heightfield.width.saturating_sub(1) as f32)
                as usize;
            let nearest_z = ((center_world_z - window.min_z()) / sample_spacing - 0.5)
                .round()
                .clamp(0.0, heightfield.height.saturating_sub(1) as f32)
                as usize;
            bottom_y = visible_surface_plan_column_height(
                &surface_plan.columns[nearest_z * heightfield.width as usize + nearest_x],
            );
            sampled_columns = 1;
        }

        Ok(Self {
            center_world_x,
            center_world_z,
            min_grid_x,
            max_grid_x,
            min_grid_z,
            max_grid_z,
            bottom_y,
            height_blocks: PLAYER_CUBE_HEIGHT_BLOCKS,
            sampled_columns,
        })
    }

    fn top_y(self) -> f32 {
        self.bottom_y + self.height_blocks
    }
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
            return Err(cli_error(
                "surface plan preview cannot render an empty tile",
            ));
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
        let mut plan = Self {
            width,
            height,
            quarter_turns: quarter_turns % 4,
            tile_w_px,
            tile_h_px,
            vertical_px_per_block: tile_h_px,
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

    fn project_grid(self, x: f32, z: f32, y_blocks: f32, tile: &HeightfieldTile) -> Point2 {
        let center_x = tile.width as f32 * 0.5;
        let center_z = tile.height as f32 * 0.5;
        let (rx, rz) = rotate_grid_delta(self.quarter_turns, x - center_x, z - center_z);
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

    fn untranslated_bounds(self, tile: &HeightfieldTile) -> (f32, f32, f32, f32) {
        let mut bounds_plan = self;
        bounds_plan.offset_x_px = 0.0;
        bounds_plan.offset_y_px = 0.0;
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        for x in [0.0, tile.width as f32] {
            for z in [0.0, tile.height as f32] {
                for y in [
                    self.min_surface_blocks,
                    self.max_surface_blocks + PLAYER_CUBE_HEIGHT_BLOCKS,
                    0.0,
                ] {
                    let p = bounds_plan.project_grid(x, z, y, tile);
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

fn render_surface_plan_isometric(
    surface_plan: &SurfacePlanArea,
    heightfield: &HeightfieldTile,
    plan: IsoRenderPlan,
    player_cube: PlayerDiagnosticCube,
) -> Result<RgbaImage, Box<dyn Error>> {
    let mut image = RgbaImage::from_pixel(plan.width, plan.height, image::Rgba([12, 15, 18, 255]));
    let width = heightfield.width as usize;
    let mut column_order = sorted_column_draw_order(heightfield, plan);
    for (_, x, z) in column_order.drain(..) {
        let index = z * width + x;
        let column = &surface_plan.columns[index];
        draw_surface_column_terrain(&mut image, surface_plan, heightfield, plan, x, z, column);
        draw_surface_column_water(&mut image, surface_plan, heightfield, plan, x, z, column);
    }
    draw_player_diagnostic_cube(&mut image, heightfield, plan, player_cube);
    Ok(image)
}

fn sorted_column_draw_order(
    tile: &HeightfieldTile,
    plan: IsoRenderPlan,
) -> Vec<(f32, usize, usize)> {
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
    draw_order
}

fn draw_surface_column_terrain(
    image: &mut RgbaImage,
    surface_plan: &SurfacePlanArea,
    heightfield: &HeightfieldTile,
    plan: IsoRenderPlan,
    x: usize,
    z: usize,
    column: &SurfaceColumnPlan,
) {
    let top_y = column.surface_y as f32;
    let top_color = material_color(&format!("{:?}", column.top_block), 255);
    let base_color = material_color(&format!("{:?}", column.base_block), 255);
    for side in visible_side_directions(plan.quarter_turns) {
        let neighbor_height = neighbor_surface_y(surface_plan, heightfield, x, z, side.dx, side.dz)
            .unwrap_or(top_y - 12.0);
        if top_y > neighbor_height + 0.75 {
            draw_side_face(
                image,
                heightfield,
                plan,
                side_edge(x, z, side.dx, side.dz),
                neighbor_height,
                top_y,
                shade_rgba(base_color, side.shade),
            );
        }
    }
    draw_top_face(
        image,
        heightfield,
        plan,
        x,
        z,
        top_y,
        shade_rgba(top_color, 1.05),
    );
}

fn draw_surface_column_water(
    image: &mut RgbaImage,
    surface_plan: &SurfacePlanArea,
    heightfield: &HeightfieldTile,
    plan: IsoRenderPlan,
    x: usize,
    z: usize,
    column: &SurfaceColumnPlan,
) {
    let Some(water_y) = column.water_y else {
        return;
    };
    if water_y < column.surface_y {
        return;
    }
    let water = water_y as f32;
    let color = [48, 104, 150, WATER_ALPHA];
    for side in visible_side_directions(plan.quarter_turns) {
        let neighbor_water = neighbor_water_y(surface_plan, heightfield, x, z, side.dx, side.dz);
        let lower_y = neighbor_water
            .unwrap_or(column.surface_y as f32)
            .max(column.surface_y as f32);
        if water > lower_y + 0.01 {
            draw_side_face(
                image,
                heightfield,
                plan,
                side_edge(x, z, side.dx, side.dz),
                lower_y,
                water,
                shade_rgba(color, side.shade),
            );
        }
    }
    draw_top_face(image, heightfield, plan, x, z, water, color);
}

fn draw_player_diagnostic_cube(
    image: &mut RgbaImage,
    tile: &HeightfieldTile,
    plan: IsoRenderPlan,
    cube: PlayerDiagnosticCube,
) {
    let side_edges = [
        (
            VisibleSide {
                dx: 1,
                dz: 0,
                shade: 0.86,
            },
            [
                cube.max_grid_x,
                cube.min_grid_z,
                cube.max_grid_x,
                cube.max_grid_z,
            ],
            PLAYER_CUBE_SIDE_A_COLOR,
        ),
        (
            VisibleSide {
                dx: -1,
                dz: 0,
                shade: 0.82,
            },
            [
                cube.min_grid_x,
                cube.min_grid_z,
                cube.min_grid_x,
                cube.max_grid_z,
            ],
            PLAYER_CUBE_SIDE_A_COLOR,
        ),
        (
            VisibleSide {
                dx: 0,
                dz: 1,
                shade: 0.78,
            },
            [
                cube.min_grid_x,
                cube.max_grid_z,
                cube.max_grid_x,
                cube.max_grid_z,
            ],
            PLAYER_CUBE_SIDE_B_COLOR,
        ),
        (
            VisibleSide {
                dx: 0,
                dz: -1,
                shade: 0.80,
            },
            [
                cube.min_grid_x,
                cube.min_grid_z,
                cube.max_grid_x,
                cube.min_grid_z,
            ],
            PLAYER_CUBE_SIDE_B_COLOR,
        ),
    ];
    let visible = visible_side_directions(plan.quarter_turns);
    for side in visible {
        if let Some((_, edge, color)) = side_edges
            .iter()
            .find(|(candidate, _, _)| candidate.dx == side.dx && candidate.dz == side.dz)
        {
            draw_side_face(
                image,
                tile,
                plan,
                *edge,
                cube.bottom_y,
                cube.top_y(),
                shade_rgba(*color, side.shade),
            );
        }
    }
    draw_rect_top_face(
        image,
        tile,
        plan,
        [
            cube.min_grid_x,
            cube.min_grid_z,
            cube.max_grid_x,
            cube.max_grid_z,
        ],
        cube.top_y(),
        PLAYER_CUBE_TOP_COLOR,
    );
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

fn iso_cardinal_screen_delta(quarter_turns: u8, dx: f32, dz: f32) -> Point2 {
    let (rx, rz) = rotate_grid_delta(quarter_turns, dx, dz);
    Point2 {
        x: (rx - rz) * 0.5,
        y: (rx + rz) * 0.5,
    }
}

fn neighbor_surface_y(
    surface_plan: &SurfacePlanArea,
    heightfield: &HeightfieldTile,
    x: usize,
    z: usize,
    dx: isize,
    dz: isize,
) -> Option<f32> {
    let nx = x.checked_add_signed(dx)?;
    let nz = z.checked_add_signed(dz)?;
    if nx >= heightfield.width as usize || nz >= heightfield.height as usize {
        return None;
    }
    Some(surface_plan.columns[nz * heightfield.width as usize + nx].surface_y as f32)
}

fn neighbor_water_y(
    surface_plan: &SurfacePlanArea,
    heightfield: &HeightfieldTile,
    x: usize,
    z: usize,
    dx: isize,
    dz: isize,
) -> Option<f32> {
    let nx = x.checked_add_signed(dx)?;
    let nz = z.checked_add_signed(dz)?;
    if nx >= heightfield.width as usize || nz >= heightfield.height as usize {
        return None;
    }
    surface_plan.columns[nz * heightfield.width as usize + nx]
        .water_y
        .map(|value| value as f32)
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

fn draw_rect_top_face(
    image: &mut RgbaImage,
    tile: &HeightfieldTile,
    plan: IsoRenderPlan,
    rect: [f32; 4],
    y: f32,
    color: [u8; 4],
) {
    let polygon = [
        plan.project_grid(rect[0], rect[1], y, tile),
        plan.project_grid(rect[2], rect[1], y, tile),
        plan.project_grid(rect[2], rect[3], y, tile),
        plan.project_grid(rect[0], rect[3], y, tile),
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

fn draw_noisy_boundary_overlay(
    image: &mut RgbaImage,
    boundary: &BoundaryCache,
    surface_plan: &SurfacePlanArea,
    heightfield: &HeightfieldTile,
    plan: IsoRenderPlan,
    window: PreviewWindow,
) {
    if heightfield.width == 0 || heightfield.height == 0 {
        return;
    }

    for curve in &boundary.curves {
        for segment in curve.points.windows(2) {
            let Some((clipped_start, clipped_end)) =
                clip_world_segment_to_window(segment[0], segment[1], window)
            else {
                continue;
            };
            let grid_start = world_point_to_tile_grid(clipped_start, window, heightfield);
            let grid_end = world_point_to_tile_grid(clipped_end, window, heightfield);
            draw_draped_noisy_boundary_segment(
                image,
                surface_plan,
                heightfield,
                plan,
                grid_start,
                grid_end,
            );
        }
    }
}

fn draw_draped_noisy_boundary_segment(
    image: &mut RgbaImage,
    surface_plan: &SurfacePlanArea,
    heightfield: &HeightfieldTile,
    plan: IsoRenderPlan,
    grid_start: WorldPlanePoint,
    grid_end: WorldPlanePoint,
) {
    let dx = grid_end.x - grid_start.x;
    let dz = grid_end.z - grid_start.z;
    let steps = dx.abs().max(dz.abs()).ceil().max(1.0) as i32;
    let mut previous = grid_start;
    let mut previous_y = noisy_boundary_overlay_height_at_grid(surface_plan, heightfield, previous);

    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        let current = WorldPlanePoint::new(grid_start.x + dx * t, grid_start.z + dz * t);
        let current_y = noisy_boundary_overlay_height_at_grid(surface_plan, heightfield, current);
        let projected_start = plan.project_grid(previous.x, previous.z, previous_y, heightfield);
        let projected_end = plan.project_grid(current.x, current.z, current_y, heightfield);
        draw_projected_line_thick(
            image,
            projected_start,
            projected_end,
            NOISY_BOUNDARY_BACKING,
            1,
            2,
        );
        draw_projected_line_thick(
            image,
            projected_start,
            projected_end,
            NOISY_BOUNDARY_CYAN,
            1,
            1,
        );
        previous = current;
        previous_y = current_y;
    }
}

fn noisy_boundary_overlay_height_at_grid(
    surface_plan: &SurfacePlanArea,
    heightfield: &HeightfieldTile,
    grid: WorldPlanePoint,
) -> f32 {
    visible_surface_plan_column_height(nearest_surface_plan_column(surface_plan, heightfield, grid))
        + NOISY_BOUNDARY_SURFACE_LIFT_BLOCKS
}

fn nearest_surface_plan_column<'a>(
    surface_plan: &'a SurfacePlanArea,
    heightfield: &HeightfieldTile,
    grid: WorldPlanePoint,
) -> &'a SurfaceColumnPlan {
    let max_x = heightfield.width.saturating_sub(1) as f32;
    let max_z = heightfield.height.saturating_sub(1) as f32;
    let x = grid.x.floor().clamp(0.0, max_x) as usize;
    let z = grid.z.floor().clamp(0.0, max_z) as usize;
    &surface_plan.columns[z * heightfield.width as usize + x]
}

fn visible_surface_plan_column_height(column: &SurfaceColumnPlan) -> f32 {
    let surface_y = column.surface_y as f32;
    column
        .water_y
        .map(|water_y| surface_y.max(water_y as f32))
        .unwrap_or(surface_y)
}

fn world_point_to_tile_grid(
    point: WorldPlanePoint,
    window: PreviewWindow,
    tile: &HeightfieldTile,
) -> WorldPlanePoint {
    let spacing_x = (window.max_x() - window.min_x()) / tile.width as f32;
    let spacing_z = (window.max_z() - window.min_z()) / tile.height as f32;
    WorldPlanePoint::new(
        (point.x - window.min_x()) / spacing_x,
        (point.z - window.min_z()) / spacing_z,
    )
}

fn clip_world_segment_to_window(
    start: WorldPlanePoint,
    end: WorldPlanePoint,
    window: PreviewWindow,
) -> Option<(WorldPlanePoint, WorldPlanePoint)> {
    let dx = end.x - start.x;
    let dz = end.z - start.z;
    let mut t_min: f32 = 0.0;
    let mut t_max: f32 = 1.0;
    let planes = [
        (-dx, start.x - window.min_x()),
        (dx, window.max_x() - start.x),
        (-dz, start.z - window.min_z()),
        (dz, window.max_z() - start.z),
    ];

    for (p, q) in planes {
        if p.abs() <= f32::EPSILON {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let t = q / p;
        if p < 0.0 {
            if t > t_max {
                return None;
            }
            t_min = t_min.max(t);
        } else {
            if t < t_min {
                return None;
            }
            t_max = t_max.min(t);
        }
    }

    Some((
        WorldPlanePoint::new(start.x + dx * t_min, start.z + dz * t_min),
        WorldPlanePoint::new(start.x + dx * t_max, start.z + dz * t_max),
    ))
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

fn material_color(name: &str, alpha: u8) -> [u8; 4] {
    let normalized = name.trim_matches('"').to_ascii_lowercase();
    let rgb = if normalized.contains("water") || normalized.contains("ice") {
        [64, 126, 166]
    } else if normalized.contains("stone")
        || normalized.contains("rock")
        || normalized.contains("sandstone")
    {
        [142, 145, 138]
    } else if normalized.contains("sand") {
        [197, 176, 112]
    } else if normalized.contains("mud") || normalized.contains("peat") {
        [88, 76, 58]
    } else if normalized.contains("soil")
        || normalized.contains("dirt")
        || normalized.contains("humus")
    {
        [104, 92, 68]
    } else if normalized.contains("silt") || normalized.contains("clay") {
        [126, 114, 94]
    } else if normalized.contains("gravel") || normalized.contains("moraine") {
        [126, 128, 120]
    } else if normalized.contains("snow") {
        [220, 228, 224]
    } else if normalized.contains("grass")
        || normalized.contains("moss")
        || normalized.contains("podzol")
    {
        [98, 136, 84]
    } else {
        stable_fallback_color(&normalized)
    };
    [rgb[0], rgb[1], rgb[2], alpha]
}

fn stable_fallback_color(value: &str) -> [u8; 3] {
    let mut hash = 0x811c9dc5u32;
    for byte in value.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    [
        80 + (hash & 0x5f) as u8,
        80 + ((hash >> 8) & 0x5f) as u8,
        80 + ((hash >> 16) & 0x5f) as u8,
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

fn blend_rgba(image: &mut RgbaImage, x: u32, y: u32, color: [u8; 4], amount: f32) {
    if x >= image.width() || y >= image.height() {
        return;
    }
    let base = image.get_pixel(x, y).0;
    let amount = amount.clamp(0.0, 1.0);
    image.put_pixel(
        x,
        y,
        image::Rgba([
            ((base[0] as f32 * (1.0 - amount)) + color[0] as f32 * amount) as u8,
            ((base[1] as f32 * (1.0 - amount)) + color[1] as f32 * amount) as u8,
            ((base[2] as f32 * (1.0 - amount)) + color[2] as f32 * amount) as u8,
            255,
        ]),
    );
}

fn write_rgba_png_with_metadata(
    image: &RgbaImage,
    output: &Path,
    header: &PreviewHeader,
) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(output)?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, image.width(), image.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.add_itxt_chunk(
        "new-world-preview-header".to_string(),
        header.to_metadata_text(),
    )?;
    encoder.add_text_chunk(
        "Software".to_string(),
        "new-world surface_plan_preview".to_string(),
    )?;
    let mut writer = encoder.write_header()?;
    writer.write_image_data(image.as_raw())?;
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
    let center_chunk_x = parse_required::<i32>(&mut args, "center-x")?;
    let center_chunk_z = parse_required::<i32>(&mut args, "center-z")?;
    let mut config = PreviewConfig {
        seed,
        center_chunk_x,
        center_chunk_z,
        chunk_radius: DEFAULT_CHUNK_RADIUS,
        width: DEFAULT_IMAGE_WIDTH,
        height: DEFAULT_IMAGE_HEIGHT,
        region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
        site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
        land_bias: MacroMapConfig::new(seed, WorldMeta::new(seed).generator_version).land_bias,
        quarter_turns: 0,
        perlin: true,
        output: None,
    };

    while !args.is_empty() {
        let flag = args.remove(0);
        match flag.as_str() {
            "--chunk-radius" => {
                config.chunk_radius = parse_required::<i32>(&mut args, "chunk-radius")?
            }
            "--width" => config.width = parse_required::<u32>(&mut args, "width")?,
            "--height" => config.height = parse_required::<u32>(&mut args, "height")?,
            "--region-size-blocks" => {
                config.region_size_blocks = parse_required::<i32>(&mut args, "region-size-blocks")?
            }
            "--site-spacing-blocks" => {
                config.site_spacing_blocks =
                    parse_required::<i32>(&mut args, "site-spacing-blocks")?
            }
            "--land-bias" => config.land_bias = parse_required::<f32>(&mut args, "land-bias")?,
            "--quarter-turns" => {
                config.quarter_turns = parse_required::<u8>(&mut args, "quarter-turns")?
            }
            "--perlin" => {
                config.perlin = true;
            }
            "--output" => {
                config.output = Some(PathBuf::from(parse_required::<String>(
                    &mut args, "output",
                )?))
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
    "usage: cargo run --bin surface_plan_preview -- <seed> <center-chunk-x> <center-chunk-z> [--chunk-radius <i32>] [--width <u32>] [--height <u32>] [--region-size-blocks <i32>] [--site-spacing-blocks <i32>] [--land-bias <f32>] [--quarter-turns <u8>] [--perlin] [--output <path>]"
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
    fn chunk_radius_maps_to_one_column_per_world_block() {
        let config = parse_args_from(["42", "2", "-3", "--chunk-radius", "1"]).unwrap();
        let window = config.window();

        assert_eq!(window.min_chunk_x, 1);
        assert_eq!(window.max_chunk_x, 3);
        assert_eq!(window.min_chunk_z, -4);
        assert_eq!(window.max_chunk_z, -2);
        assert_eq!(window.columns_x, 96);
        assert_eq!(window.columns_z, 96);
        assert_eq!(window.min_world_x, 32);
        assert_eq!(window.min_world_z, -128);
    }

    #[test]
    fn default_output_path_uses_surface_plan_folder() {
        let config = parse_args_from(["42", "0", "0", "--quarter-turns", "5"]).unwrap();

        assert_eq!(
            config.output_path(),
            PathBuf::from("target/surface-plan-preview/s42_cx0_cz0_q1_r0.png")
        );
    }

    #[test]
    fn parse_perlin_flag_is_backward_compatible_noop_alias() {
        let default_config = parse_args_from(["42", "0", "0"]).unwrap();
        let enabled_config = parse_args_from(["42", "0", "0", "--perlin"]).unwrap();
        let meta = WorldMeta::new(42);
        let default_heightfield = preview_heightfield_config(default_config.perlin, &meta);
        let enabled_heightfield = preview_heightfield_config(enabled_config.perlin, &meta);

        assert!(default_config.perlin);
        assert!(default_heightfield.perlin.enabled);
        assert!(enabled_config.perlin);
        assert!(enabled_heightfield.perlin.enabled);
        assert_eq!(default_heightfield.perlin.seed, meta.seed);
        assert_eq!(
            default_heightfield.perlin.generator_version,
            meta.generator_version
        );
        assert_eq!(enabled_heightfield.perlin.seed, meta.seed);
        assert_eq!(
            enabled_heightfield.perlin.generator_version,
            meta.generator_version
        );
    }

    #[test]
    fn material_color_groups_common_surface_keys() {
        assert_ne!(material_color("sand", 255), material_color("grass", 255));
        assert_ne!(material_color("stone", 255), material_color("water", 255));
        assert_eq!(material_color("\"mud\"", 255), material_color("mud", 255));
        assert_eq!(
            material_color("sandstone", 255),
            material_color("stone", 255)
        );
        assert_ne!(material_color("dirt", 255), material_color("grass", 255));
        assert_ne!(
            material_color("thin_soil", 255),
            material_color("grass", 255)
        );
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
    }
}
