use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{self, BufWriter, ErrorKind};
use std::path::{Path, PathBuf};
use std::time::Instant;

use image::RgbImage;
use rayon::prelude::*;

use new_world::world::WorldMeta;
use new_world::world::generation::{
    BoundaryCache, BoundaryConfig, DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
    DEFAULT_MACRO_FIELD_CONTOUR_MAJOR_EVERY, DEFAULT_MACRO_FIELD_CONTOUR_STEP_BLOCKS,
    DEFAULT_SITE_SPACING_BLOCKS, GraphHydrologyGraph, GraphMacroMap, GraphRegionArea,
    GraphRegionCoord, HydrologyConfig, MACRO_FIELD_CONTOUR_HEIGHT_MAX_BLOCKS,
    MACRO_FIELD_CONTOUR_HEIGHT_MIN_BLOCKS, MacroFieldContourSet,
    MacroFieldSample as CoreMacroFieldSample, MacroFieldTileConfig as CoreMacroFieldTileConfig,
    MacroFieldTileStats as CoreMacroFieldTileStats, MacroMapConfig, VoronoiGraphConfig,
    VoronoiGraphPatch, VoronoiGraphPatchRequest, WorldPlanePoint, extract_macro_field_contours,
    generate_macro_field_tile, generate_macro_map, generate_noisy_boundaries,
    generate_voronoi_graph_patch, graph_region_for_world_block, solve_hydrology,
};

mod common;

use common::preview_compass::draw_compass_rgb;

const DEFAULT_WIDTH: u32 = 3840;
const DEFAULT_HEIGHT: u32 = 2160;
const DEFAULT_WORLD_SPAN_BLOCKS: i32 = 32768;
const DEFAULT_STAGE: &str = "macro_field";
const OUTPUT_DIR: &str = "target/macro-field-preview";
const MACRO_PREVIEW_MIN_HEIGHT: f32 = -1.0;
const MACRO_PREVIEW_MAX_HEIGHT: f32 = 1.0;
const COMBINED_PREVIEW_MIN_HEIGHT: f32 = -0.75;
const COMBINED_PREVIEW_MAX_HEIGHT: f32 = 1.25;
const WHITE_SATURATION_THRESHOLD: u8 = 248;
const LIT_NORMAL_SAMPLE_RADIUS: usize = 8;
const LIT_NORMAL_PREFILTER_RADIUS: usize = 5;
const LIT_NORMAL_SLOPE_SCALE: f32 = 7.25;
const LIT_DIFFUSE_STRENGTH: f32 = 0.44;
const LIT_AMBIENT_BASE: f32 = 0.20;
const LIT_HEIGHT_STRENGTH: f32 = 0.44;
const LIT_MAX_SHADE: f32 = 0.96;
const GRAPH_EDGE_OVERLAY_COLOR: [u8; 3] = [8, 11, 15];
const GRAPH_EDGE_OVERLAY_AMOUNT: f32 = 0.075;
const LIT_GRAPH_EDGE_OVERLAY_AMOUNT: f32 = 0.025;
const TILE_GRID_OVERLAY_AMOUNT: f32 = 0.18;
const MASK_OCEAN_COLOR: [u8; 3] = [31, 90, 164];
const MASK_LAKE_COLOR: [u8; 3] = [54, 150, 198];
const MASK_DRY_BASIN_COLOR: [u8; 3] = [122, 105, 129];
const MASK_COAST_COLOR: [u8; 3] = [220, 196, 125];
const MASK_LAND_COLOR: [u8; 3] = [101, 154, 89];
const CONTOUR_SEA_COLOR: [u8; 3] = [96, 165, 204];
const CONTOUR_MINOR_AMOUNT: f32 = 0.68;
const CONTOUR_MAJOR_AMOUNT: f32 = 0.88;
const CONTOUR_SEA_AMOUNT: f32 = 0.92;
const COMBINED_CONTOUR_AMOUNT_SCALE: f32 = 0.72;
const LIT_CONTOUR_AMOUNT_SCALE: f32 = 0.48;
const RENDERABLE_CHANNELS: [PreviewChannel; 7] = [
    PreviewChannel::MacroElevation,
    PreviewChannel::Mask,
    PreviewChannel::RidgeInfluence,
    PreviewChannel::RiverValley,
    PreviewChannel::CombinedMacroHeight,
    PreviewChannel::LitHeightfield,
    PreviewChannel::Contour,
];

#[derive(Debug, Clone, PartialEq)]
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
    stage: String,
    channel: PreviewChannelSelection,
    contour_step_blocks: f32,
    contour_major_every: u32,
    contours: bool,
    output: Option<PathBuf>,
}

impl PreviewConfig {
    fn validate(self) -> Result<Self, Box<dyn Error>> {
        if self.width == 0 || self.height == 0 {
            return Err(cli_error("width and height must be >= 1"));
        }
        if self.world_span_blocks <= 0 {
            return Err(cli_error("world-span-blocks must be positive"));
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
        if !self.contour_step_blocks.is_finite() || self.contour_step_blocks <= 0.0 {
            return Err(cli_error("contour-step must be finite and positive"));
        }
        if self.contour_major_every == 0 {
            return Err(cli_error("contour-major-every must be >= 1"));
        }
        Ok(self)
    }

    fn window(&self) -> PreviewWindow {
        PreviewWindow {
            center_x: self.center_x as f32,
            center_z: self.center_z as f32,
            width: self.width,
            height: self.height,
            world_span_x: self.world_span_blocks as f32,
            world_span_z: self.world_span_blocks as f32 * self.height as f32 / self.width as f32,
        }
    }

    fn default_single_path(&self, channel: PreviewChannel) -> PathBuf {
        PathBuf::from(format!(
            "{OUTPUT_DIR}/s{}_x{}_z{}_{}.png",
            self.seed,
            self.center_x,
            self.center_z,
            channel.as_str()
        ))
    }

    fn default_all_dir(&self) -> PathBuf {
        PathBuf::from(format!(
            "{OUTPUT_DIR}/s{}_x{}_z{}",
            self.seed, self.center_x, self.center_z
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewChannelSelection {
    All,
    Single(PreviewChannel),
}

impl PreviewChannelSelection {
    fn channels(self) -> &'static [PreviewChannel] {
        match self {
            Self::All => &RENDERABLE_CHANNELS,
            Self::Single(PreviewChannel::MacroElevation) => &RENDERABLE_CHANNELS[0..1],
            Self::Single(PreviewChannel::Mask) => &RENDERABLE_CHANNELS[1..2],
            Self::Single(PreviewChannel::RidgeInfluence) => &RENDERABLE_CHANNELS[2..3],
            Self::Single(PreviewChannel::RiverValley) => &RENDERABLE_CHANNELS[3..4],
            Self::Single(PreviewChannel::CombinedMacroHeight) => &RENDERABLE_CHANNELS[4..5],
            Self::Single(PreviewChannel::LitHeightfield) => &RENDERABLE_CHANNELS[5..6],
            Self::Single(PreviewChannel::Contour) => &RENDERABLE_CHANNELS[6..7],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewChannel {
    MacroElevation,
    Mask,
    RidgeInfluence,
    RiverValley,
    CombinedMacroHeight,
    LitHeightfield,
    Contour,
}

impl PreviewChannel {
    fn parse(value: &str) -> Option<PreviewChannelSelection> {
        match value {
            "all" => Some(PreviewChannelSelection::All),
            "macro" | "macro_elevation" | "elevation" => {
                Some(PreviewChannelSelection::Single(Self::MacroElevation))
            }
            "mask" | "masks" | "coast_ocean_lake" => {
                Some(PreviewChannelSelection::Single(Self::Mask))
            }
            "ridge" | "ridge_influence" => {
                Some(PreviewChannelSelection::Single(Self::RidgeInfluence))
            }
            "river" | "river_valley" => Some(PreviewChannelSelection::Single(Self::RiverValley)),
            "combined" | "combined_macro_height" => {
                Some(PreviewChannelSelection::Single(Self::CombinedMacroHeight))
            }
            "lit" | "heightfield" | "lit_heightfield" => {
                Some(PreviewChannelSelection::Single(Self::LitHeightfield))
            }
            "contour" | "contours" => Some(PreviewChannelSelection::Single(Self::Contour)),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::MacroElevation => "macro",
            Self::Mask => "mask",
            Self::RidgeInfluence => "ridge",
            Self::RiverValley => "river",
            Self::CombinedMacroHeight => "combined",
            Self::LitHeightfield => "lit",
            Self::Contour => "contour",
        }
    }

    fn map_name(self) -> &'static str {
        match self {
            Self::MacroElevation => "macro elevation field",
            Self::Mask => "coast ocean lake mask",
            Self::RidgeInfluence => "ridge influence field",
            Self::RiverValley => "river valley field",
            Self::CombinedMacroHeight => "combined macro height",
            Self::LitHeightfield => "lit heightfield preview",
            Self::Contour => "combined macro height contours",
        }
    }

    fn legend_title(self) -> &'static str {
        match self {
            Self::MacroElevation => "MACRO",
            Self::Mask => "MASK",
            Self::RidgeInfluence => "RIDGE",
            Self::RiverValley => "RIVER",
            Self::CombinedMacroHeight => "COMBINED",
            Self::LitHeightfield => "LIT H",
            Self::Contour => "CONTOUR",
        }
    }

    fn legend_min_label(self) -> &'static str {
        match self {
            Self::MacroElevation => "LOW",
            Self::Mask => "OCEAN",
            Self::RidgeInfluence => "NONE",
            Self::RiverValley => "NONE",
            Self::CombinedMacroHeight => "LOW",
            Self::LitHeightfield => "SHADE",
            Self::Contour => "MINOR",
        }
    }

    fn legend_max_label(self) -> &'static str {
        match self {
            Self::MacroElevation => "HIGH",
            Self::Mask => "LAND",
            Self::RidgeInfluence => "RIDGE",
            Self::RiverValley => "VALLEY",
            Self::CombinedMacroHeight => "HIGH",
            Self::LitHeightfield => "LIGHT",
            Self::Contour => "MAJOR",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PreviewWindow {
    center_x: f32,
    center_z: f32,
    width: u32,
    height: u32,
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
        GraphRegionArea::new(min, max).ok_or_else(|| cli_error("invalid macro field preview area"))
    }
}

#[derive(Debug, Clone)]
struct PreviewWorld {
    patch: VoronoiGraphPatch,
    macro_map: GraphMacroMap,
    hydrology: GraphHydrologyGraph,
    boundary: BoundaryCache,
    river_segment_count: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct FieldSample {
    macro_elevation: f32,
    ocean_mask: f32,
    lake_mask: f32,
    coast_mask: f32,
    dry_mask: f32,
    ridge_influence: f32,
    river_valley: f32,
    combined_height: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct ChannelStats {
    min: f32,
    max: f32,
    average: f32,
    robust_min: f32,
    robust_max: f32,
    contrast_span: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct TileGridStats {
    spacing_blocks: f32,
    vertical_lines: usize,
    horizontal_lines: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct GraphEdgeOverlayStats {
    noisy_curve_count: usize,
    drawn_segment_count: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct ScaleBarStats {
    length_blocks: f32,
    length_pixels: u32,
}

#[derive(Debug, Clone, Copy, Default)]
struct LitGradientStats {
    raw_average: f32,
    raw_max: f32,
    smoothed_average: f32,
    smoothed_max: f32,
    brightness_min: f32,
    brightness_average: f32,
    brightness_max: f32,
    brightness_stddev: f32,
}

#[derive(Debug, Clone)]
struct MacroFieldTile {
    samples: Vec<FieldSample>,
    macro_stats: ChannelStats,
    ridge_stats: ChannelStats,
    river_stats: ChannelStats,
    combined_stats: ChannelStats,
    core_stats: CoreMacroFieldTileStats,
    contours: MacroFieldContourSet,
}

#[derive(Debug, Clone)]
struct PreviewHeader {
    seed: u64,
    generator_version: u32,
    stage: String,
    channel: PreviewChannel,
    center_x: i32,
    center_z: i32,
    width: u32,
    height: u32,
    world_span_blocks: i32,
    region_size_blocks: i32,
    site_spacing_blocks: i32,
    land_bias: f32,
    graph_area: GraphRegionArea,
    site_count: usize,
    macro_edge_count: usize,
    river_segment_count: usize,
    boundary_curve_count: usize,
    average_boundary_displacement_blocks: f32,
    max_boundary_displacement_blocks: f32,
    ridge_feature_samples: usize,
    river_feature_samples: usize,
    coast_feature_samples: usize,
    tile_generation_ms: u128,
    render_encode_ms: u128,
    total_runtime_ms: u128,
    tile_boundary_spacing_blocks: f32,
    tile_boundary_vertical_lines: usize,
    tile_boundary_horizontal_lines: usize,
    graph_edge_overlay_curve_count: usize,
    graph_edge_overlay_segment_count: usize,
    scale_bar_length_blocks: f32,
    scale_bar_length_pixels: u32,
    contour_step_blocks: f32,
    contour_major_every: u32,
    contour_min_level_blocks: f32,
    contour_max_level_blocks: f32,
    contour_level_count: usize,
    contour_segment_count: usize,
    contour_overlay_enabled: bool,
    lit_raw_gradient_average: f32,
    lit_raw_gradient_max: f32,
    lit_smoothed_gradient_average: f32,
    lit_smoothed_gradient_max: f32,
    lit_brightness_min: f32,
    lit_brightness_average: f32,
    lit_brightness_max: f32,
    lit_brightness_stddev: f32,
    white_saturation_fraction: f32,
    macro_stats: ChannelStats,
    ridge_stats: ChannelStats,
    river_stats: ChannelStats,
    combined_stats: ChannelStats,
    core_stats: CoreMacroFieldTileStats,
}

impl PreviewHeader {
    fn to_metadata_text(&self) -> String {
        [
            "binary=macro_field_preview".to_string(),
            format!("seed={}", self.seed),
            format!("generator_version={}", self.generator_version),
            format!("stage={}", self.stage),
            format!("channel={}", self.channel.as_str()),
            format!("map_name={}", self.channel.map_name()),
            format!("center_x={}", self.center_x),
            format!("center_z={}", self.center_z),
            format!("width={}", self.width),
            format!("height={}", self.height),
            "orientation_overlay=north_up_east_right".to_string(),
            format!("world_span_blocks={}", self.world_span_blocks),
            format!("region_size_blocks={}", self.region_size_blocks),
            format!("site_spacing_blocks={}", self.site_spacing_blocks),
            format!("land_bias={}", self.land_bias),
            format!(
                "graph_area_min={},{}",
                self.graph_area.min.x, self.graph_area.min.z
            ),
            format!(
                "graph_area_max={},{}",
                self.graph_area.max.x, self.graph_area.max.z
            ),
            format!("site_count={}", self.site_count),
            format!("macro_edge_count={}", self.macro_edge_count),
            format!("river_segment_count={}", self.river_segment_count),
            format!("boundary_curve_count={}", self.boundary_curve_count),
            format!(
                "boundary_displacement_avg_max_blocks={:.4},{:.4}",
                self.average_boundary_displacement_blocks, self.max_boundary_displacement_blocks
            ),
            format!("ridge_feature_samples={}", self.ridge_feature_samples),
            format!("river_feature_samples={}", self.river_feature_samples),
            format!("coast_feature_samples={}", self.coast_feature_samples),
            format!(
                "ridge_active_samples_fraction={},{:.4}",
                self.core_stats.ridge_active_sample_count,
                fraction(
                    self.core_stats.ridge_active_sample_count,
                    self.core_stats.sample_count
                )
            ),
            format!(
                "dry_basin_samples_height_min_max_avg={},{:.4},{:.4},{:.4}",
                self.core_stats.dry_basin_sample_count,
                self.core_stats.min_dry_basin_height,
                self.core_stats.max_dry_basin_height,
                self.core_stats.average_dry_basin_height
            ),
            format!("tile_generation_ms={}", self.tile_generation_ms),
            format!("render_encode_ms={}", self.render_encode_ms),
            format!("total_runtime_ms={}", self.total_runtime_ms),
            format!(
                "preview_height_scale=absolute_normalized_macro_{:.2}_to_{:.2}_combined_{:.2}_to_{:.2}",
                MACRO_PREVIEW_MIN_HEIGHT,
                MACRO_PREVIEW_MAX_HEIGHT,
                COMBINED_PREVIEW_MIN_HEIGHT,
                COMBINED_PREVIEW_MAX_HEIGHT
            ),
            format!(
                "tile_boundary_overlay=macro_field_cache_tile_grid_spacing_blocks_{:.1}",
                self.tile_boundary_spacing_blocks
            ),
            format!(
                "tile_boundary_lines_vertical_horizontal={},{}",
                self.tile_boundary_vertical_lines, self.tile_boundary_horizontal_lines
            ),
            format!(
                "voronoi_noisy_edge_overlay_curves_segments={},{}",
                self.graph_edge_overlay_curve_count, self.graph_edge_overlay_segment_count
            ),
            format!(
                "voronoi_noisy_edge_overlay_style=color_{:02x}{:02x}{:02x}_amount_{:.3}_lit_amount_{:.3}",
                GRAPH_EDGE_OVERLAY_COLOR[0],
                GRAPH_EDGE_OVERLAY_COLOR[1],
                GRAPH_EDGE_OVERLAY_COLOR[2],
                GRAPH_EDGE_OVERLAY_AMOUNT,
                LIT_GRAPH_EDGE_OVERLAY_AMOUNT
            ),
            format!(
                "scale_bar_blocks_pixels={:.1},{}",
                self.scale_bar_length_blocks, self.scale_bar_length_pixels
            ),
            format!(
                "contour_step_major_every={:.2},{}",
                self.contour_step_blocks, self.contour_major_every
            ),
            format!(
                "contour_min_max_level_blocks={:.2},{:.2}",
                self.contour_min_level_blocks, self.contour_max_level_blocks
            ),
            format!(
                "contour_levels_segments={},{}",
                self.contour_level_count, self.contour_segment_count
            ),
            format!("contour_overlay_enabled={}", self.contour_overlay_enabled),
            format!(
                "lit_gradient_raw_avg_max={:.6},{:.6}",
                self.lit_raw_gradient_average, self.lit_raw_gradient_max
            ),
            format!(
                "lit_gradient_smoothed_avg_max={:.6},{:.6}",
                self.lit_smoothed_gradient_average, self.lit_smoothed_gradient_max
            ),
            format!(
                "lit_broad_hillshade_brightness_min_avg_max_stddev={:.6},{:.6},{:.6},{:.6}",
                self.lit_brightness_min,
                self.lit_brightness_average,
                self.lit_brightness_max,
                self.lit_brightness_stddev
            ),
            format!(
                "white_saturation_fraction_channel={:.6}",
                self.white_saturation_fraction
            ),
            format!(
                "macro_elevation_min_max_avg={:.4},{:.4},{:.4}",
                self.macro_stats.min, self.macro_stats.max, self.macro_stats.average
            ),
            format!(
                "ridge_influence_min_max_avg={:.4},{:.4},{:.4}",
                self.ridge_stats.min, self.ridge_stats.max, self.ridge_stats.average
            ),
            format!(
                "river_valley_min_max_avg={:.4},{:.4},{:.4}",
                self.river_stats.min, self.river_stats.max, self.river_stats.average
            ),
            format!(
                "combined_height_min_max_avg={:.4},{:.4},{:.4}",
                self.combined_stats.min, self.combined_stats.max, self.combined_stats.average
            ),
            format!(
                "macro_elevation_robust_min_max_contrast={:.4},{:.4},{:.4}",
                self.macro_stats.robust_min,
                self.macro_stats.robust_max,
                self.macro_stats.contrast_span
            ),
            format!(
                "combined_height_robust_min_max_contrast={:.4},{:.4},{:.4}",
                self.combined_stats.robust_min,
                self.combined_stats.robust_max,
                self.combined_stats.contrast_span
            ),
            "macro_field_meaning=graph_macro_hydrology_boundary_raster_cache_for_heightfield"
                .to_string(),
            "macro_elevation=noisy_boundary_owner_blended_signed_elevation".to_string(),
            "mask=ocean_lake_coast_dry_land_context_following_noisy_boundaries".to_string(),
            "ridge_influence=distance_to_ridge_noisy_edge_envelope".to_string(),
            "river_valley=distance_to_selected_river_noisy_edge_envelope".to_string(),
            "combined=macro_elevation_plus_ridge_minus_river_and_water_flatten".to_string(),
            "contour=block_height_marching_squares_from_combined_macro_height_before_heightfield"
                .to_string(),
            "lit=topdown_white_heightfield_shaded_from_combined_height_gradient".to_string(),
        ]
        .join("\n")
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let total_start = Instant::now();
    let config = parse_args()?.validate()?;
    let meta = WorldMeta::new(config.seed);
    let window = config.window();
    let graph_area = window.graph_area(config.region_size_blocks)?;
    let build_start = Instant::now();
    let preview = build_preview_world(&meta, &config, graph_area)?;
    let build_ms = build_start.elapsed().as_millis();
    let tile_start = Instant::now();
    let tile = rasterize_macro_field(window, &preview, &config)?;
    let tile_generation_ms = tile_start.elapsed().as_millis();
    let output_paths = output_paths_for_config(&config)?;
    let mut generated = Vec::with_capacity(output_paths.len());
    let render_start = Instant::now();
    let tile_grid = tile_grid_stats(window, config.region_size_blocks as f32);
    let edge_overlay = graph_edge_overlay_stats(window, &preview.boundary);
    let scale_bar = scale_bar_stats(window);
    let lit_gradient = lit_gradient_stats(&tile, config.width as usize, config.height as usize);

    for (channel, output) in output_paths {
        let white_saturation_fraction = channel_white_saturation_fraction(
            &tile,
            channel,
            config.width as usize,
            config.height as usize,
        );
        let header = PreviewHeader {
            seed: config.seed,
            generator_version: meta.generator_version,
            stage: config.stage.clone(),
            channel,
            center_x: config.center_x,
            center_z: config.center_z,
            width: config.width,
            height: config.height,
            world_span_blocks: config.world_span_blocks,
            region_size_blocks: config.region_size_blocks,
            site_spacing_blocks: config.site_spacing_blocks,
            land_bias: config.land_bias,
            graph_area,
            site_count: preview.patch.sites.len(),
            macro_edge_count: preview.macro_map.edges.len(),
            river_segment_count: preview.river_segment_count,
            boundary_curve_count: preview.boundary.curves.len(),
            average_boundary_displacement_blocks: preview
                .boundary
                .stats
                .average_perpendicular_displacement_blocks,
            max_boundary_displacement_blocks: preview
                .boundary
                .stats
                .max_perpendicular_displacement_blocks,
            ridge_feature_samples: tile.core_stats.ridge_source_pixel_count,
            river_feature_samples: tile.core_stats.river_source_pixel_count,
            coast_feature_samples: tile.core_stats.coast_source_pixel_count,
            tile_generation_ms,
            render_encode_ms: render_start.elapsed().as_millis(),
            total_runtime_ms: total_start.elapsed().as_millis(),
            tile_boundary_spacing_blocks: tile_grid.spacing_blocks,
            tile_boundary_vertical_lines: tile_grid.vertical_lines,
            tile_boundary_horizontal_lines: tile_grid.horizontal_lines,
            graph_edge_overlay_curve_count: edge_overlay.noisy_curve_count,
            graph_edge_overlay_segment_count: edge_overlay.drawn_segment_count,
            scale_bar_length_blocks: scale_bar.length_blocks,
            scale_bar_length_pixels: scale_bar.length_pixels,
            contour_step_blocks: tile.contours.step_blocks,
            contour_major_every: tile.contours.major_every,
            contour_min_level_blocks: tile.contours.min_level_blocks,
            contour_max_level_blocks: tile.contours.max_level_blocks,
            contour_level_count: tile.contours.levels.len(),
            contour_segment_count: tile.contours.total_segment_count,
            contour_overlay_enabled: config.contours,
            lit_raw_gradient_average: lit_gradient.raw_average,
            lit_raw_gradient_max: lit_gradient.raw_max,
            lit_smoothed_gradient_average: lit_gradient.smoothed_average,
            lit_smoothed_gradient_max: lit_gradient.smoothed_max,
            lit_brightness_min: lit_gradient.brightness_min,
            lit_brightness_average: lit_gradient.brightness_average,
            lit_brightness_max: lit_gradient.brightness_max,
            lit_brightness_stddev: lit_gradient.brightness_stddev,
            white_saturation_fraction,
            macro_stats: tile.macro_stats,
            ridge_stats: tile.ridge_stats,
            river_stats: tile.river_stats,
            combined_stats: tile.combined_stats,
            core_stats: tile.core_stats,
        };
        let mut image = render_channel(window, &tile, channel)?;
        draw_tile_boundary_overlay(&mut image, window, tile_grid);
        if channel != PreviewChannel::Contour {
            draw_noisy_graph_edge_overlay(&mut image, window, &preview.boundary, channel);
        }
        if channel == PreviewChannel::Contour
            || (config.contours
                && matches!(
                    channel,
                    PreviewChannel::CombinedMacroHeight | PreviewChannel::LitHeightfield
                ))
        {
            draw_contour_overlay(&mut image, window, &tile.contours, channel);
        }
        draw_scale_bar_overlay(&mut image, window, scale_bar);
        draw_legend_overlay(
            &mut image,
            channel,
            config.contour_step_blocks,
            config.contour_major_every,
        );
        draw_compass_rgb(&mut image);
        write_png_with_metadata(&image, &output, &header)?;
        generated.push((channel, output, image.width(), image.height()));
    }
    let render_encode_ms = render_start.elapsed().as_millis();
    let total_runtime_ms = total_start.elapsed().as_millis();

    println!("seed: {}", config.seed);
    println!("generator version: {}", meta.generator_version);
    println!("stage: {}", config.stage);
    println!(
        "channels: {}",
        config
            .channel
            .channels()
            .iter()
            .map(|channel| channel.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "center world block: ({}, {})",
        config.center_x, config.center_z
    );
    println!(
        "world footprint: x={:.1}..{:.1}, z={:.1}..{:.1}",
        window.min_x(),
        window.max_x(),
        window.min_z(),
        window.max_z()
    );
    println!(
        "graph regions: x={}..{}, z={}..{}",
        graph_area.min.x, graph_area.max.x, graph_area.min.z, graph_area.max.z
    );
    println!(
        "stage inputs: sites {}, macro edges {}, boundary curves {}, selected river segments {}, river source pixels {}",
        preview.patch.sites.len(),
        preview.macro_map.edges.len(),
        preview.boundary.curves.len(),
        preview.river_segment_count,
        tile.core_stats.river_source_pixel_count
    );
    println!(
        "macro field splat sources: ridge curves/pixels {}/{}, river curves/pixels {}/{}, coast curves/pixels {}/{}",
        tile.core_stats.ridge_source_curve_count,
        tile.core_stats.ridge_source_pixel_count,
        tile.core_stats.river_source_curve_count,
        tile.core_stats.river_source_pixel_count,
        tile.core_stats.coast_source_curve_count,
        tile.core_stats.coast_source_pixel_count
    );
    println!(
        "timing: build world {} ms, macro field tile {} ms, render+encode {} ms, total {} ms",
        build_ms, tile_generation_ms, render_encode_ms, total_runtime_ms
    );
    println!(
        "lit gradient raw avg/max {:.6}/{:.6}, smoothed normal avg/max {:.6}/{:.6}",
        lit_gradient.raw_average,
        lit_gradient.raw_max,
        lit_gradient.smoothed_average,
        lit_gradient.smoothed_max
    );
    println!(
        "lit broad hillshade brightness min/avg/max/stddev {:.3}/{:.3}/{:.3}/{:.3}",
        lit_gradient.brightness_min,
        lit_gradient.brightness_average,
        lit_gradient.brightness_max,
        lit_gradient.brightness_stddev
    );
    println!(
        "preview overlays: noisy voronoi curves {}, drawn segments {}, graph edge amount {:.3} (lit {:.3}), cache grid v/h {}/{}, scale bar {:.0} blocks ({} px)",
        edge_overlay.noisy_curve_count,
        edge_overlay.drawn_segment_count,
        GRAPH_EDGE_OVERLAY_AMOUNT,
        LIT_GRAPH_EDGE_OVERLAY_AMOUNT,
        tile_grid.vertical_lines,
        tile_grid.horizontal_lines,
        scale_bar.length_blocks,
        scale_bar.length_pixels
    );
    println!(
        "noisy boundary displacement avg/max {:.2}/{:.2} blocks",
        preview
            .boundary
            .stats
            .average_perpendicular_displacement_blocks,
        preview.boundary.stats.max_perpendicular_displacement_blocks
    );
    println!(
        "macro elevation min/avg/max {:.3}/{:.3}/{:.3}",
        tile.macro_stats.min, tile.macro_stats.average, tile.macro_stats.max
    );
    println!(
        "ridge influence min/avg/max {:.3}/{:.3}/{:.3}",
        tile.ridge_stats.min, tile.ridge_stats.average, tile.ridge_stats.max
    );
    println!(
        "ridge active samples {} / {} ({:.3})",
        tile.core_stats.ridge_active_sample_count,
        tile.core_stats.sample_count,
        fraction(
            tile.core_stats.ridge_active_sample_count,
            tile.core_stats.sample_count
        )
    );
    println!(
        "river valley min/avg/max {:.3}/{:.3}/{:.3}",
        tile.river_stats.min, tile.river_stats.average, tile.river_stats.max
    );
    println!(
        "combined height min/avg/max {:.3}/{:.3}/{:.3}",
        tile.combined_stats.min, tile.combined_stats.average, tile.combined_stats.max
    );
    println!(
        "absolute preview scale: macro {:.2}..{:.2}, combined {:.2}..{:.2}",
        MACRO_PREVIEW_MIN_HEIGHT,
        MACRO_PREVIEW_MAX_HEIGHT,
        COMBINED_PREVIEW_MIN_HEIGHT,
        COMBINED_PREVIEW_MAX_HEIGHT
    );
    println!(
        "contours: step {:.1} blocks, major every {}, levels {}, segments {}, level range {:.1}..{:.1}",
        tile.contours.step_blocks,
        tile.contours.major_every,
        tile.contours.levels.len(),
        tile.contours.total_segment_count,
        tile.contours.min_level_blocks,
        tile.contours.max_level_blocks
    );
    println!(
        "tile boundary overlay: spacing {:.1} blocks, vertical lines {}, horizontal lines {}",
        tile_grid.spacing_blocks, tile_grid.vertical_lines, tile_grid.horizontal_lines
    );
    for channel in config.channel.channels() {
        println!(
            "{} white saturation fraction {:.4}",
            channel.as_str(),
            channel_white_saturation_fraction(
                &tile,
                *channel,
                config.width as usize,
                config.height as usize
            )
        );
    }
    println!(
        "dry basin samples {} height min/avg/max {:.3}/{:.3}/{:.3}",
        tile.core_stats.dry_basin_sample_count,
        tile.core_stats.min_dry_basin_height,
        tile.core_stats.average_dry_basin_height,
        tile.core_stats.max_dry_basin_height
    );
    println!(
        "preview contrast macro robust {:.3}..{:.3} span {:.3}; combined robust {:.3}..{:.3} span {:.3}",
        tile.macro_stats.robust_min,
        tile.macro_stats.robust_max,
        tile.macro_stats.contrast_span,
        tile.combined_stats.robust_min,
        tile.combined_stats.robust_max,
        tile.combined_stats.contrast_span
    );
    println!("metadata: new-world-preview-header iTXt chunk");
    println!();
    println!("generated files:");
    for (channel, output, width, height) in generated {
        println!(
            "{} {}x{} {}",
            channel.as_str(),
            width,
            height,
            output.display()
        );
    }

    Ok(())
}

fn parse_args() -> Result<PreviewConfig, Box<dyn Error>> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() < 3 {
        return Err(cli_error(usage()));
    }

    let seed = parse_required::<u64>(&mut args, "seed")?;
    let center_x = parse_required::<i32>(&mut args, "center-x")?;
    let center_z = parse_required::<i32>(&mut args, "center-z")?;
    let mut width = DEFAULT_WIDTH;
    let mut height = DEFAULT_HEIGHT;
    let mut world_span_blocks = DEFAULT_WORLD_SPAN_BLOCKS;
    let mut region_size_blocks = DEFAULT_GRAPH_REGION_SIZE_BLOCKS;
    let mut site_spacing_blocks = DEFAULT_SITE_SPACING_BLOCKS;
    let mut land_bias = MacroMapConfig::new(seed, WorldMeta::new(seed).generator_version).land_bias;
    let mut stage = DEFAULT_STAGE.to_string();
    let mut channel = PreviewChannelSelection::Single(PreviewChannel::LitHeightfield);
    let mut contour_step_blocks = DEFAULT_MACRO_FIELD_CONTOUR_STEP_BLOCKS;
    let mut contour_major_every = DEFAULT_MACRO_FIELD_CONTOUR_MAJOR_EVERY;
    let mut contours = false;
    let mut output = None;

    while let Some(flag) = args.first().cloned() {
        args.remove(0);
        match flag.as_str() {
            "--width" => width = parse_required::<u32>(&mut args, "width")?,
            "--height" => height = parse_required::<u32>(&mut args, "height")?,
            "--world-span-blocks" => {
                world_span_blocks = parse_required::<i32>(&mut args, "world-span-blocks")?
            }
            "--region-size-blocks" => {
                region_size_blocks = parse_required::<i32>(&mut args, "region-size-blocks")?
            }
            "--site-spacing-blocks" => {
                site_spacing_blocks = parse_required::<i32>(&mut args, "site-spacing-blocks")?
            }
            "--land-bias" => land_bias = parse_required::<f32>(&mut args, "land-bias")?,
            "--stage" => stage = parse_required::<String>(&mut args, "stage")?,
            "--contour-step" => {
                contour_step_blocks = parse_required::<f32>(&mut args, "contour-step")?
            }
            "--contour-major-every" => {
                contour_major_every = parse_required::<u32>(&mut args, "contour-major-every")?
            }
            "--contours" => contours = true,
            "--channel" => {
                let value = parse_required::<String>(&mut args, "channel")?;
                channel = PreviewChannel::parse(&value).ok_or_else(|| {
                    cli_error(format!("unsupported channel: {value}\n\n{}", usage()))
                })?;
            }
            "--output" => {
                output = Some(PathBuf::from(parse_required::<String>(
                    &mut args, "output",
                )?))
            }
            _ => return Err(cli_error(format!("unknown flag: {flag}\n\n{}", usage()))),
        }
    }

    Ok(PreviewConfig {
        seed,
        center_x,
        center_z,
        width,
        height,
        world_span_blocks,
        region_size_blocks,
        site_spacing_blocks,
        land_bias,
        stage,
        channel,
        contour_step_blocks,
        contour_major_every,
        contours,
        output,
    })
}

fn output_paths_for_config(
    config: &PreviewConfig,
) -> Result<Vec<(PreviewChannel, PathBuf)>, Box<dyn Error>> {
    let channels = config.channel.channels();
    if matches!(config.channel, PreviewChannelSelection::All) {
        let output_dir = config.output.as_ref().map_or_else(
            || config.default_all_dir(),
            |path| {
                if looks_like_file(path) {
                    let parent = path.parent().unwrap_or_else(|| Path::new("."));
                    let stem = path.file_stem().unwrap_or_default();
                    parent.join(stem)
                } else {
                    path.clone()
                }
            },
        );
        return Ok(channels
            .iter()
            .map(|channel| {
                (
                    *channel,
                    output_dir.join(format!("{}.png", channel.as_str())),
                )
            })
            .collect());
    }

    let channel = channels[0];
    let output = config.output.as_ref().map_or_else(
        || config.default_single_path(channel),
        |path| {
            if looks_like_file(path) {
                path.clone()
            } else {
                path.join(format!("{}.png", channel.as_str()))
            }
        },
    );
    Ok(vec![(channel, output)])
}

fn looks_like_file(path: &Path) -> bool {
    path.extension().is_some()
}

fn build_preview_world(
    meta: &WorldMeta,
    config: &PreviewConfig,
    graph_area: GraphRegionArea,
) -> Result<PreviewWorld, Box<dyn Error>> {
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
    let request = VoronoiGraphPatchRequest::new(graph_config, config.center_x, config.center_z);
    let patch = generate_voronoi_graph_patch(request);
    if patch.sites.is_empty() {
        return Err(cli_error("generated graph patch did not contain sites"));
    }

    let macro_map = generate_macro_map(
        &patch,
        MacroMapConfig {
            land_bias: config.land_bias,
            ..MacroMapConfig::new(meta.seed, meta.generator_version)
        },
    );
    let hydrology = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());
    let boundary = generate_noisy_boundaries(
        &patch,
        &macro_map,
        BoundaryConfig::new(meta.seed, meta.generator_version),
    );
    let river_segment_count = hydrology.segments.len();

    Ok(PreviewWorld {
        patch,
        macro_map,
        hydrology,
        boundary,
        river_segment_count,
    })
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
        .map_err(|_| cli_error("macro field preview padding overflowed"))
}

fn rasterize_macro_field(
    window: PreviewWindow,
    preview: &PreviewWorld,
    config: &PreviewConfig,
) -> Result<MacroFieldTile, Box<dyn Error>> {
    let sample_spacing = window.world_span_x / window.width as f32;
    let core_config = CoreMacroFieldTileConfig::new(
        window.min_x() + sample_spacing * 0.5,
        window.min_z() + sample_spacing * 0.5,
        window.width,
        window.height,
        sample_spacing,
    );
    let core_tile = generate_macro_field_tile(
        &preview.patch,
        &preview.macro_map,
        &preview.hydrology,
        &preview.boundary,
        core_config,
    );
    let contours = extract_macro_field_contours(
        &core_tile,
        config.contour_step_blocks,
        config.contour_major_every,
    );
    let samples = core_tile
        .samples
        .iter()
        .copied()
        .map(FieldSample::from_core)
        .collect::<Vec<_>>();

    Ok(MacroFieldTile {
        macro_stats: channel_stats(&samples, |sample| sample.macro_elevation),
        ridge_stats: channel_stats(&samples, |sample| sample.ridge_influence),
        river_stats: channel_stats(&samples, |sample| sample.river_valley),
        combined_stats: channel_stats(&samples, |sample| sample.combined_height),
        core_stats: core_tile.stats,
        contours,
        samples,
    })
}

impl FieldSample {
    fn from_core(sample: CoreMacroFieldSample) -> Self {
        Self {
            macro_elevation: sample.macro_elevation,
            ocean_mask: sample.ocean_mask,
            lake_mask: sample.lake_mask,
            coast_mask: sample.coast_mask,
            dry_mask: sample.dry_basin_mask,
            ridge_influence: sample.ridge_influence,
            river_valley: sample.river_valley_strength,
            combined_height: sample.combined_macro_height,
        }
    }
}

fn channel_stats(samples: &[FieldSample], read: fn(&FieldSample) -> f32) -> ChannelStats {
    if samples.is_empty() {
        return ChannelStats::default();
    }
    let mut min = f32::MAX;
    let mut max = f32::MIN;
    let mut sum = 0.0;
    let mut values = Vec::with_capacity(samples.len());
    for sample in samples {
        let value = read(sample);
        min = min.min(value);
        max = max.max(value);
        sum += value;
        values.push(value);
    }
    values.sort_by(f32::total_cmp);
    let robust_min = percentile_sorted(&values, 0.02);
    let robust_max = percentile_sorted(&values, 0.98);
    ChannelStats {
        min,
        max,
        average: sum / samples.len() as f32,
        robust_min,
        robust_max,
        contrast_span: robust_max - robust_min,
    }
}

fn percentile_sorted(values: &[f32], t: f32) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let index = ((values.len() - 1) as f32 * t.clamp(0.0, 1.0)).round() as usize;
    values[index]
}

fn fraction(count: usize, total: usize) -> f32 {
    if total == 0 {
        0.0
    } else {
        count as f32 / total as f32
    }
}

fn render_channel(
    window: PreviewWindow,
    tile: &MacroFieldTile,
    channel: PreviewChannel,
) -> Result<RgbImage, Box<dyn Error>> {
    let width = usize::try_from(window.width).map_err(|_| cli_error("image width overflowed"))?;
    let height =
        usize::try_from(window.height).map_err(|_| cli_error("image height overflowed"))?;
    let len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or_else(|| cli_error("image dimensions overflowed"))?;
    let mut pixels = vec![0_u8; len];

    pixels
        .par_chunks_mut(3)
        .enumerate()
        .for_each(|(index, pixel)| {
            let x = index % width;
            let y = index / width;
            let color = color_for_channel(tile, channel, x, y, width, height);
            pixel.copy_from_slice(&color);
        });

    RgbImage::from_raw(window.width, window.height, pixels)
        .ok_or_else(|| cli_error("failed to build RGB image"))
}

fn tile_grid_stats(window: PreviewWindow, spacing_blocks: f32) -> TileGridStats {
    let spacing = spacing_blocks.max(1.0);
    TileGridStats {
        spacing_blocks: spacing,
        vertical_lines: grid_line_count(window.min_x(), window.max_x(), spacing),
        horizontal_lines: grid_line_count(window.min_z(), window.max_z(), spacing),
    }
}

fn grid_line_count(min: f32, max: f32, spacing: f32) -> usize {
    let first = (min / spacing).ceil() as i32;
    let last = (max / spacing).floor() as i32;
    if last < first {
        0
    } else {
        (last - first + 1) as usize
    }
}

fn graph_edge_overlay_stats(
    window: PreviewWindow,
    boundary: &BoundaryCache,
) -> GraphEdgeOverlayStats {
    let drawn_segment_count = boundary
        .curves
        .iter()
        .flat_map(|curve| curve.points.windows(2))
        .filter(|segment| clip_world_segment_to_window(segment[0], segment[1], window).is_some())
        .count();

    GraphEdgeOverlayStats {
        noisy_curve_count: boundary.curves.len(),
        drawn_segment_count,
    }
}

fn scale_bar_stats(window: PreviewWindow) -> ScaleBarStats {
    let target_blocks = window.world_span_x * 0.16;
    let length_blocks = nice_scale_bar_length(target_blocks);
    let length_pixels = (length_blocks / window.world_span_x * window.width as f32)
        .round()
        .max(1.0) as u32;

    ScaleBarStats {
        length_blocks,
        length_pixels,
    }
}

fn nice_scale_bar_length(target_blocks: f32) -> f32 {
    const CANDIDATES: [f32; 11] = [
        128.0, 256.0, 512.0, 1024.0, 2048.0, 4096.0, 8192.0, 16384.0, 32768.0, 65536.0, 131072.0,
    ];
    CANDIDATES
        .into_iter()
        .filter(|candidate| *candidate <= target_blocks.max(128.0))
        .last()
        .unwrap_or(128.0)
}

fn draw_tile_boundary_overlay(image: &mut RgbImage, window: PreviewWindow, grid: TileGridStats) {
    if grid.spacing_blocks <= 0.0 {
        return;
    }
    let color = [248, 244, 214];
    let major_color = [255, 255, 255];
    let first_x = (window.min_x() / grid.spacing_blocks).ceil() as i32;
    let last_x = (window.max_x() / grid.spacing_blocks).floor() as i32;
    for gx in first_x..=last_x {
        let world_x = gx as f32 * grid.spacing_blocks;
        let px = ((world_x - window.min_x()) / window.world_span_x * image.width() as f32).round();
        if !(0.0..image.width() as f32).contains(&px) {
            continue;
        }
        let amount = if gx == 0 {
            TILE_GRID_OVERLAY_AMOUNT * 1.45
        } else {
            TILE_GRID_OVERLAY_AMOUNT
        };
        draw_vertical_line(
            image,
            px as u32,
            if gx == 0 { major_color } else { color },
            amount,
        );
    }

    let first_z = (window.min_z() / grid.spacing_blocks).ceil() as i32;
    let last_z = (window.max_z() / grid.spacing_blocks).floor() as i32;
    for gz in first_z..=last_z {
        let world_z = gz as f32 * grid.spacing_blocks;
        let py = ((world_z - window.min_z()) / window.world_span_z * image.height() as f32).round();
        if !(0.0..image.height() as f32).contains(&py) {
            continue;
        }
        let amount = if gz == 0 {
            TILE_GRID_OVERLAY_AMOUNT * 1.45
        } else {
            TILE_GRID_OVERLAY_AMOUNT
        };
        draw_horizontal_line(
            image,
            py as u32,
            if gz == 0 { major_color } else { color },
            amount,
        );
    }
}

fn draw_noisy_graph_edge_overlay(
    image: &mut RgbImage,
    window: PreviewWindow,
    boundary: &BoundaryCache,
    channel: PreviewChannel,
) {
    let amount = graph_edge_overlay_amount(channel);
    for curve in &boundary.curves {
        for segment in curve.points.windows(2) {
            let Some((start, end)) = clip_world_segment_to_window(segment[0], segment[1], window)
            else {
                continue;
            };
            let (sx, sy) = world_to_pixel(start, window, image.width(), image.height());
            let (ex, ey) = world_to_pixel(end, window, image.width(), image.height());
            draw_pixel_line(image, sx, sy, ex, ey, GRAPH_EDGE_OVERLAY_COLOR, amount);
        }
    }
}

fn graph_edge_overlay_amount(channel: PreviewChannel) -> f32 {
    if channel == PreviewChannel::LitHeightfield {
        LIT_GRAPH_EDGE_OVERLAY_AMOUNT
    } else if channel == PreviewChannel::Contour {
        0.0
    } else {
        GRAPH_EDGE_OVERLAY_AMOUNT
    }
}

fn draw_contour_overlay(
    image: &mut RgbImage,
    window: PreviewWindow,
    contours: &MacroFieldContourSet,
    channel: PreviewChannel,
) {
    let amount_scale = match channel {
        PreviewChannel::LitHeightfield => LIT_CONTOUR_AMOUNT_SCALE,
        PreviewChannel::CombinedMacroHeight => COMBINED_CONTOUR_AMOUNT_SCALE,
        _ => 1.0,
    };
    for level in &contours.levels {
        let sea_level = level.height_blocks.abs() <= contours.step_blocks * 0.5;
        let color = if sea_level {
            CONTOUR_SEA_COLOR
        } else {
            contour_height_color(level.height_blocks, level.is_major)
        };
        let amount = if sea_level {
            CONTOUR_SEA_AMOUNT
        } else if level.is_major {
            CONTOUR_MAJOR_AMOUNT
        } else {
            CONTOUR_MINOR_AMOUNT
        } * amount_scale;
        for segment in &level.segments {
            let Some((start, end)) =
                clip_world_segment_to_window(segment.start, segment.end, window)
            else {
                continue;
            };
            let (sx, sy) = world_to_pixel(start, window, image.width(), image.height());
            let (ex, ey) = world_to_pixel(end, window, image.width(), image.height());
            draw_pixel_line(image, sx, sy, ex, ey, color, amount);
            if level.is_major || sea_level {
                draw_pixel_line(image, sx + 1, sy, ex + 1, ey, color, amount * 0.72);
            }
        }
    }
}

fn draw_scale_bar_overlay(image: &mut RgbImage, _window: PreviewWindow, stats: ScaleBarStats) {
    if image.width() < 80 || image.height() < 48 || stats.length_pixels == 0 {
        return;
    }

    let scale = if image.width() >= 640 && image.height() >= 360 {
        2
    } else {
        1
    };
    let margin = 12 * scale;
    let bar_width = stats
        .length_pixels
        .min(image.width().saturating_sub(margin * 2));
    if bar_width < 8 {
        return;
    }

    let panel_width = (bar_width + 24 * scale).min(image.width());
    let panel_height = (28 * scale).min(image.height());
    let x = image.width().saturating_sub(panel_width + margin);
    let y = image.height().saturating_sub(panel_height + margin);
    let bar_x = x + 12 * scale;
    let bar_y = y + 10 * scale;
    let label = scale_bar_label(stats.length_blocks);

    blend_rect(image, x, y, panel_width, panel_height, [8, 11, 14], 0.62);
    draw_scale_bar_line(image, bar_x, bar_y, bar_width, [242, 245, 235], 0.92);
    draw_text(
        image,
        bar_x,
        bar_y + 7 * scale,
        &label,
        [230, 235, 222],
        scale,
    );
}

fn draw_scale_bar_line(
    image: &mut RgbImage,
    x: u32,
    y: u32,
    width: u32,
    color: [u8; 3],
    amount: f32,
) {
    for px in x..=(x + width).min(image.width().saturating_sub(1)) {
        blend_pixel(image, px, y, color, amount);
        blend_pixel(image, px, y + 1, color, amount);
    }
    for tick_x in [x, (x + width).min(image.width().saturating_sub(1))] {
        for py in y.saturating_sub(3)..=(y + 4).min(image.height().saturating_sub(1)) {
            blend_pixel(image, tick_x, py, color, amount);
            if tick_x + 1 < image.width() {
                blend_pixel(image, tick_x + 1, py, color, amount);
            }
        }
    }
}

fn scale_bar_label(length_blocks: f32) -> String {
    if length_blocks >= 1024.0 {
        format!("{}K BLOCKS", (length_blocks / 1024.0).round() as u32)
    } else {
        format!("{} BLOCKS", length_blocks.round() as u32)
    }
}

fn world_to_pixel(
    point: WorldPlanePoint,
    window: PreviewWindow,
    width: u32,
    height: u32,
) -> (i32, i32) {
    let x = ((point.x - window.min_x()) / window.world_span_x * width as f32).round() as i32;
    let y = ((point.z - window.min_z()) / window.world_span_z * height as f32).round() as i32;
    (x, y)
}

fn clip_world_segment_to_window(
    start: WorldPlanePoint,
    end: WorldPlanePoint,
    window: PreviewWindow,
) -> Option<(WorldPlanePoint, WorldPlanePoint)> {
    let mut t0 = 0.0;
    let mut t1 = 1.0;
    let dx = end.x - start.x;
    let dz = end.z - start.z;

    if !clip_axis(-dx, start.x - window.min_x(), &mut t0, &mut t1) {
        return None;
    }
    if !clip_axis(dx, window.max_x() - start.x, &mut t0, &mut t1) {
        return None;
    }
    if !clip_axis(-dz, start.z - window.min_z(), &mut t0, &mut t1) {
        return None;
    }
    if !clip_axis(dz, window.max_z() - start.z, &mut t0, &mut t1) {
        return None;
    }

    Some((
        WorldPlanePoint::new(start.x + dx * t0, start.z + dz * t0),
        WorldPlanePoint::new(start.x + dx * t1, start.z + dz * t1),
    ))
}

fn clip_axis(p: f32, q: f32, t0: &mut f32, t1: &mut f32) -> bool {
    if p.abs() <= f32::EPSILON {
        return q >= 0.0;
    }
    let r = q / p;
    if p < 0.0 {
        if r > *t1 {
            return false;
        }
        if r > *t0 {
            *t0 = r;
        }
    } else {
        if r < *t0 {
            return false;
        }
        if r < *t1 {
            *t1 = r;
        }
    }
    true
}

fn draw_pixel_line(
    image: &mut RgbImage,
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
    color: [u8; 3],
    amount: f32,
) {
    let mut x = start_x;
    let mut y = start_y;
    let dx = (end_x - start_x).abs();
    let dy = -(end_y - start_y).abs();
    let sx = if start_x < end_x { 1 } else { -1 };
    let sy = if start_y < end_y { 1 } else { -1 };
    let mut error = dx + dy;

    loop {
        if x >= 0 && y >= 0 {
            blend_pixel(image, x as u32, y as u32, color, amount);
        }
        if x == end_x && y == end_y {
            break;
        }
        let e2 = 2 * error;
        if e2 >= dy {
            error += dy;
            x += sx;
        }
        if e2 <= dx {
            error += dx;
            y += sy;
        }
    }
}

fn draw_vertical_line(image: &mut RgbImage, x: u32, color: [u8; 3], amount: f32) {
    for y in 0..image.height() {
        blend_pixel(image, x, y, color, amount);
    }
}

fn draw_horizontal_line(image: &mut RgbImage, y: u32, color: [u8; 3], amount: f32) {
    for x in 0..image.width() {
        blend_pixel(image, x, y, color, amount);
    }
}

fn channel_white_saturation_fraction(
    tile: &MacroFieldTile,
    channel: PreviewChannel,
    width: usize,
    height: usize,
) -> f32 {
    if width == 0 || height == 0 {
        return 0.0;
    }
    let saturated = (0..width * height)
        .into_par_iter()
        .filter(|index| {
            let x = index % width;
            let y = index / width;
            let color = color_for_channel(tile, channel, x, y, width, height);
            color
                .iter()
                .all(|channel| *channel >= WHITE_SATURATION_THRESHOLD)
        })
        .count();
    fraction(saturated, width * height)
}

fn color_for_channel(
    tile: &MacroFieldTile,
    channel: PreviewChannel,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) -> [u8; 3] {
    let index = y * width + x;
    let sample = tile.samples[index];
    match channel {
        PreviewChannel::MacroElevation => {
            gradient_macro(normalize_absolute_macro_height(sample.macro_elevation))
        }
        PreviewChannel::Mask => color_for_mask(sample),
        PreviewChannel::RidgeInfluence => gradient_fire(sample.ridge_influence),
        PreviewChannel::RiverValley => gradient_river(sample.river_valley),
        PreviewChannel::CombinedMacroHeight => {
            combined_terrain_ramp(normalize_absolute_combined_height(sample.combined_height))
        }
        PreviewChannel::LitHeightfield => lit_height_color(tile, x, y, width, height),
        PreviewChannel::Contour => contour_background_color(sample),
    }
}

fn contour_background_color(sample: FieldSample) -> [u8; 3] {
    let base = combined_terrain_ramp(normalize_absolute_combined_height(sample.combined_height));
    blend(base, [34, 36, 38], 0.58)
}

fn color_for_mask(sample: FieldSample) -> [u8; 3] {
    if sample.ocean_mask > 0.5 {
        return MASK_OCEAN_COLOR;
    }
    if sample.lake_mask > 0.5 {
        return MASK_LAKE_COLOR;
    }
    if sample.dry_mask > 0.5 {
        return MASK_DRY_BASIN_COLOR;
    }
    if sample.coast_mask > 0.45 {
        return MASK_COAST_COLOR;
    }
    MASK_LAND_COLOR
}

fn lit_height_color(
    tile: &MacroFieldTile,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) -> [u8; 3] {
    let shade = lit_height_shade(tile, x, y, width, height);
    let value = (shade * 255.0).round() as u8;
    [value, value, value]
}

fn lit_height_shade(tile: &MacroFieldTile, x: usize, y: usize, width: usize, height: usize) -> f32 {
    let radius = LIT_NORMAL_SAMPLE_RADIUS.min(width.saturating_sub(1).max(1));
    let left_x = x.saturating_sub(radius);
    let right_x = (x + radius).min(width - 1);
    let up_y = y.saturating_sub(radius);
    let down_y = (y + radius).min(height - 1);
    let dx_span = (right_x - left_x).max(1) as f32;
    let dz_span = (down_y - up_y).max(1) as f32;
    let dx = (smoothed_lit_height(tile, right_x, y, width, height)
        - smoothed_lit_height(tile, left_x, y, width, height))
        / dx_span;
    let dz = (smoothed_lit_height(tile, x, down_y, width, height)
        - smoothed_lit_height(tile, x, up_y, width, height))
        / dz_span;
    let normal = normalize3([
        -dx * LIT_NORMAL_SLOPE_SCALE,
        1.0,
        -dz * LIT_NORMAL_SLOPE_SCALE,
    ]);
    let light = normalize3([-0.45, 0.78, -0.43]);
    let diffuse = dot3(normal, light).max(0.0);
    let broad_height = smoothed_lit_height(tile, x, y, width, height);
    let height_t = normalize_absolute_combined_height(broad_height);
    (LIT_AMBIENT_BASE + height_t * LIT_HEIGHT_STRENGTH + diffuse * LIT_DIFFUSE_STRENGTH)
        .clamp(0.0, LIT_MAX_SHADE)
}

fn smoothed_lit_height(
    tile: &MacroFieldTile,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) -> f32 {
    let radius = LIT_NORMAL_PREFILTER_RADIUS;
    let min_x = x.saturating_sub(radius);
    let max_x = (x + radius).min(width - 1);
    let min_y = y.saturating_sub(radius);
    let max_y = (y + radius).min(height - 1);
    let mut weighted_sum = 0.0;
    let mut weight_sum = 0.0;

    for sample_y in min_y..=max_y {
        for sample_x in min_x..=max_x {
            let dx = sample_x.abs_diff(x) as f32;
            let dy = sample_y.abs_diff(y) as f32;
            let distance2 = dx * dx + dy * dy;
            let weight = 1.0 / (1.0 + distance2);
            weighted_sum += tile.samples[sample_y * width + sample_x].combined_height * weight;
            weight_sum += weight;
        }
    }

    weighted_sum / weight_sum.max(f32::EPSILON)
}

fn raw_height_gradient(
    tile: &MacroFieldTile,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) -> f32 {
    let left = tile.samples[y * width + x.saturating_sub(1)].combined_height;
    let right = tile.samples[y * width + (x + 1).min(width - 1)].combined_height;
    let up = tile.samples[y.saturating_sub(1) * width + x].combined_height;
    let down = tile.samples[(y + 1).min(height - 1) * width + x].combined_height;
    let dx = right - left;
    let dz = down - up;
    (dx * dx + dz * dz).sqrt()
}

fn smoothed_lit_gradient(
    tile: &MacroFieldTile,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) -> f32 {
    let radius = LIT_NORMAL_SAMPLE_RADIUS.min(width.saturating_sub(1).max(1));
    let left_x = x.saturating_sub(radius);
    let right_x = (x + radius).min(width - 1);
    let up_y = y.saturating_sub(radius);
    let down_y = (y + radius).min(height - 1);
    let dx_span = (right_x - left_x).max(1) as f32;
    let dz_span = (down_y - up_y).max(1) as f32;
    let dx = (smoothed_lit_height(tile, right_x, y, width, height)
        - smoothed_lit_height(tile, left_x, y, width, height))
        / dx_span;
    let dz = (smoothed_lit_height(tile, x, down_y, width, height)
        - smoothed_lit_height(tile, x, up_y, width, height))
        / dz_span;
    (dx * dx + dz * dz).sqrt()
}

fn lit_gradient_stats(tile: &MacroFieldTile, width: usize, height: usize) -> LitGradientStats {
    if width == 0 || height == 0 || tile.samples.is_empty() {
        return LitGradientStats::default();
    }

    let (
        raw_sum,
        raw_max,
        smoothed_sum,
        smoothed_max,
        brightness_sum,
        brightness_min,
        brightness_max,
    ) = (0..width * height)
        .into_par_iter()
        .map(|index| {
            let x = index % width;
            let y = index / width;
            let raw = raw_height_gradient(tile, x, y, width, height);
            let smoothed = smoothed_lit_gradient(tile, x, y, width, height);
            let brightness = lit_height_shade(tile, x, y, width, height);
            (
                raw, raw, smoothed, smoothed, brightness, brightness, brightness,
            )
        })
        .reduce(
            || (0.0, 0.0, 0.0, 0.0, 0.0, f32::INFINITY, f32::NEG_INFINITY),
            |left, right| {
                (
                    left.0 + right.0,
                    left.1.max(right.1),
                    left.2 + right.2,
                    left.3.max(right.3),
                    left.4 + right.4,
                    left.5.min(right.5),
                    left.6.max(right.6),
                )
            },
        );
    let count = (width * height) as f32;
    let brightness_average = brightness_sum / count;
    let brightness_variance = (0..width * height)
        .into_par_iter()
        .map(|index| {
            let x = index % width;
            let y = index / width;
            let delta = lit_height_shade(tile, x, y, width, height) - brightness_average;
            delta * delta
        })
        .sum::<f32>()
        / count;

    LitGradientStats {
        raw_average: raw_sum / count,
        raw_max,
        smoothed_average: smoothed_sum / count,
        smoothed_max,
        brightness_min,
        brightness_average,
        brightness_max,
        brightness_stddev: brightness_variance.sqrt(),
    }
}

fn normalize_absolute_macro_height(value: f32) -> f32 {
    normalize_absolute(value, MACRO_PREVIEW_MIN_HEIGHT, MACRO_PREVIEW_MAX_HEIGHT)
}

fn normalize_absolute_combined_height(value: f32) -> f32 {
    normalize_absolute(
        value,
        COMBINED_PREVIEW_MIN_HEIGHT,
        COMBINED_PREVIEW_MAX_HEIGHT,
    )
}

fn normalize_absolute(value: f32, min: f32, max: f32) -> f32 {
    let span = max - min;
    if span.abs() <= f32::EPSILON {
        0.5
    } else {
        ((value - min) / span).clamp(0.0, 1.0)
    }
}

fn gradient_macro(value: f32) -> [u8; 3] {
    gradient_color(
        value,
        &[
            (0.00, [28, 72, 132]),
            (0.38, [74, 142, 161]),
            (0.50, [211, 194, 126]),
            (0.70, [115, 151, 91]),
            (0.88, [134, 127, 116]),
            (1.00, [241, 243, 235]),
        ],
    )
}

fn combined_terrain_ramp(value: f32) -> [u8; 3] {
    gradient_color(
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
    )
}

fn contour_height_color(height_blocks: f32, is_major: bool) -> [u8; 3] {
    let normalized = ((height_blocks - MACRO_FIELD_CONTOUR_HEIGHT_MIN_BLOCKS)
        / (MACRO_FIELD_CONTOUR_HEIGHT_MAX_BLOCKS - MACRO_FIELD_CONTOUR_HEIGHT_MIN_BLOCKS))
        .clamp(0.0, 1.0);
    let base = gradient_color(
        normalized,
        &[
            (0.00, [39, 90, 154]),
            (0.22, [76, 142, 190]),
            (0.48, [214, 207, 150]),
            (0.70, [214, 125, 76]),
            (1.00, [168, 48, 45]),
        ],
    );
    if is_major { lighten(base, 0.18) } else { base }
}

fn gradient_fire(value: f32) -> [u8; 3] {
    gradient_color(
        value,
        &[
            (0.00, [18, 24, 31]),
            (0.36, [78, 78, 84]),
            (0.68, [188, 158, 91]),
            (1.00, [250, 248, 230]),
        ],
    )
}

fn gradient_river(value: f32) -> [u8; 3] {
    gradient_color(
        value,
        &[
            (0.00, [21, 27, 34]),
            (0.35, [28, 85, 130]),
            (0.72, [35, 177, 211]),
            (1.00, [217, 250, 255]),
        ],
    )
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

fn draw_legend_overlay(
    image: &mut RgbImage,
    channel: PreviewChannel,
    contour_step_blocks: f32,
    contour_major_every: u32,
) {
    if image.width() < 48 || image.height() < 28 {
        return;
    }

    let scale = if image.width() >= 640 && image.height() >= 360 {
        2
    } else {
        1
    };
    let margin = 8 * scale;
    let panel_width = (164 * scale).min(image.width());
    let panel_height_units = if channel == PreviewChannel::Mask {
        66
    } else if channel == PreviewChannel::Contour {
        62
    } else {
        50
    };
    let panel_height = (panel_height_units * scale).min(image.height());
    let x = margin.min(image.width().saturating_sub(panel_width));
    let y = margin.min(image.height().saturating_sub(panel_height));

    blend_rect(image, x, y, panel_width, panel_height, [10, 13, 18], 0.72);
    draw_text(
        image,
        x + 7 * scale,
        y + 6 * scale,
        channel.legend_title(),
        [238, 241, 232],
        scale,
    );

    let bar_x = x + 8 * scale;
    let bar_y = y + 21 * scale;
    let bar_width = panel_width.saturating_sub(16 * scale).max(1);
    let bar_height = 7 * scale;
    draw_gradient_bar(image, channel, bar_x, bar_y, bar_width, bar_height);
    draw_text(
        image,
        bar_x,
        bar_y + bar_height + 5 * scale,
        channel.legend_min_label(),
        [218, 224, 212],
        scale,
    );
    let right = channel.legend_max_label();
    let right_width = text_width(right, scale);
    draw_text(
        image,
        bar_x + bar_width.saturating_sub(right_width),
        bar_y + bar_height + 5 * scale,
        right,
        [218, 224, 212],
        scale,
    );

    if channel == PreviewChannel::Mask {
        draw_mask_legend_keys(image, bar_x, bar_y + bar_height + 17 * scale, scale);
    } else if channel == PreviewChannel::Contour {
        draw_contour_legend_keys(
            image,
            bar_x,
            bar_y + bar_height + 17 * scale,
            scale,
            contour_step_blocks,
            contour_major_every,
        );
    }
}

fn draw_contour_legend_keys(
    image: &mut RgbImage,
    x: u32,
    y: u32,
    scale: u32,
    contour_step_blocks: f32,
    contour_major_every: u32,
) {
    let keys = [
        (
            "LOW",
            contour_height_color(MACRO_FIELD_CONTOUR_HEIGHT_MIN_BLOCKS, false),
        ),
        (
            "HIGH",
            contour_height_color(MACRO_FIELD_CONTOUR_HEIGHT_MAX_BLOCKS, true),
        ),
        ("SEA", CONTOUR_SEA_COLOR),
    ];
    let mut cursor_x = x;
    for (label, color) in keys {
        draw_scale_bar_line(image, cursor_x, y + 3 * scale, 10 * scale, color, 0.9);
        draw_text(
            image,
            cursor_x + 13 * scale,
            y,
            label,
            [218, 224, 212],
            scale,
        );
        cursor_x += 36 * scale;
    }
    let label = format!(
        "{}B/{}B",
        contour_step_blocks.round() as i32,
        (contour_step_blocks * contour_major_every as f32).round() as i32
    );
    draw_text(image, x, y + 12 * scale, &label, [218, 224, 212], scale);
}

fn draw_mask_legend_keys(image: &mut RgbImage, x: u32, y: u32, scale: u32) {
    let keys = [
        ("OCN", MASK_OCEAN_COLOR),
        ("LAK", MASK_LAKE_COLOR),
        ("DRY", MASK_DRY_BASIN_COLOR),
        ("CST", MASK_COAST_COLOR),
        ("LND", MASK_LAND_COLOR),
    ];
    let mut cursor_x = x;
    for (label, color) in keys {
        let swatch = 5 * scale;
        for sy in 0..swatch {
            for sx in 0..swatch {
                set_pixel(image, cursor_x + sx, y + sy, color);
            }
        }
        draw_text(
            image,
            cursor_x + swatch + 2 * scale,
            y,
            label,
            [218, 224, 212],
            scale,
        );
        cursor_x += swatch + 15 * scale;
    }
}

fn draw_gradient_bar(
    image: &mut RgbImage,
    channel: PreviewChannel,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) {
    let max_x = (x + width).min(image.width());
    let max_y = (y + height).min(image.height());
    let denom = width.saturating_sub(1).max(1) as f32;
    for py in y..max_y {
        for px in x..max_x {
            let t = (px - x) as f32 / denom;
            let color = match channel {
                PreviewChannel::MacroElevation => gradient_macro(t),
                PreviewChannel::Mask => {
                    if t < 0.20 {
                        MASK_OCEAN_COLOR
                    } else if t < 0.40 {
                        MASK_LAKE_COLOR
                    } else if t < 0.60 {
                        MASK_DRY_BASIN_COLOR
                    } else if t < 0.80 {
                        MASK_COAST_COLOR
                    } else {
                        MASK_LAND_COLOR
                    }
                }
                PreviewChannel::RidgeInfluence => gradient_fire(t),
                PreviewChannel::RiverValley => gradient_river(t),
                PreviewChannel::CombinedMacroHeight => combined_terrain_ramp(t),
                PreviewChannel::LitHeightfield => {
                    let v = (t * 255.0).round() as u8;
                    [v, v, v]
                }
                PreviewChannel::Contour => {
                    if (t - 0.5).abs() <= 0.025 {
                        CONTOUR_SEA_COLOR
                    } else {
                        contour_height_color(
                            MACRO_FIELD_CONTOUR_HEIGHT_MIN_BLOCKS
                                + t * (MACRO_FIELD_CONTOUR_HEIGHT_MAX_BLOCKS
                                    - MACRO_FIELD_CONTOUR_HEIGHT_MIN_BLOCKS),
                            false,
                        )
                    }
                }
            };
            set_pixel(image, px, py, color);
        }
    }
}

fn blend_rect(
    image: &mut RgbImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: [u8; 3],
    amount: f32,
) {
    let max_x = (x + width).min(image.width());
    let max_y = (y + height).min(image.height());
    for py in y..max_y {
        for px in x..max_x {
            blend_pixel(image, px, py, color, amount);
        }
    }
}

fn draw_text(image: &mut RgbImage, x: u32, y: u32, text: &str, color: [u8; 3], scale: u32) {
    let mut cursor_x = x;
    for ch in text.chars() {
        draw_char(image, cursor_x, y, ch, color, scale);
        cursor_x = cursor_x.saturating_add(4 * scale);
    }
}

fn draw_char(image: &mut RgbImage, x: u32, y: u32, ch: char, color: [u8; 3], scale: u32) {
    let glyph = glyph_3x5(ch);
    for (row, bits) in glyph.iter().enumerate() {
        for col in 0..3 {
            if (bits >> (2 - col)) & 1 == 0 {
                continue;
            }
            for sy in 0..scale {
                for sx in 0..scale {
                    blend_pixel(
                        image,
                        x + col * scale + sx,
                        y + row as u32 * scale + sy,
                        color,
                        0.95,
                    );
                }
            }
        }
    }
}

fn text_width(text: &str, scale: u32) -> u32 {
    text.chars().count() as u32 * 4 * scale
}

fn glyph_3x5(ch: char) -> [u8; 5] {
    match ch {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
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
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
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
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        _ => [0b000, 0b000, 0b000, 0b000, 0b000],
    }
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
        "new-world macro_field_preview".to_string(),
    )?;
    let mut png_writer = encoder.write_header()?;
    png_writer.write_image_data(image.as_raw())?;
    Ok(())
}

fn blend_pixel(image: &mut RgbImage, x: u32, y: u32, color: [u8; 3], amount: f32) {
    if x >= image.width() || y >= image.height() {
        return;
    }
    let index = ((y as usize * image.width() as usize) + x as usize) * 3;
    let pixels: &mut [u8] = image.as_mut();
    let base = [pixels[index], pixels[index + 1], pixels[index + 2]];
    let blended = blend(base, color, amount);
    pixels[index..index + 3].copy_from_slice(&blended);
}

fn set_pixel(image: &mut RgbImage, x: u32, y: u32, color: [u8; 3]) {
    if x >= image.width() || y >= image.height() {
        return;
    }
    let index = ((y as usize * image.width() as usize) + x as usize) * 3;
    let pixels: &mut [u8] = image.as_mut();
    pixels[index..index + 3].copy_from_slice(&color);
}

fn blend(base: [u8; 3], tint: [u8; 3], amount: f32) -> [u8; 3] {
    let amount = amount.clamp(0.0, 1.0);
    [
        mix_channel(base[0], tint[0], amount),
        mix_channel(base[1], tint[1], amount),
        mix_channel(base[2], tint[2], amount),
    ]
}

fn lighten(base: [u8; 3], amount: f32) -> [u8; 3] {
    blend(base, [255, 248, 228], amount)
}

fn mix_channel(base: u8, tint: u8, amount: f32) -> u8 {
    ((base as f32 * (1.0 - amount)) + (tint as f32 * amount))
        .round()
        .clamp(0.0, 255.0) as u8
}

fn normalize3(value: [f32; 3]) -> [f32; 3] {
    let length = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2])
        .sqrt()
        .max(f32::EPSILON);
    [value[0] / length, value[1] / length, value[2] / length]
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn parse_required<T>(args: &mut Vec<String>, label: &str) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: Error + Send + Sync + 'static,
{
    let value = args
        .first()
        .cloned()
        .ok_or_else(|| cli_error(format!("missing {label}\n\n{}", usage())))?;
    args.remove(0);
    value
        .parse::<T>()
        .map_err(|error| cli_error(format!("invalid {label}: {error}")))
}

fn usage() -> &'static str {
    "usage: cargo run --bin macro_field_preview -- <seed> <center-x> <center-z> [--width <u32>] [--height <u32>] [--world-span-blocks <i32>] [--region-size-blocks <i32>] [--site-spacing-blocks <i32>] [--land-bias <f32>] [--stage macro_field] [--channel <all|macro|mask|ridge|river|combined|lit|contour>] [--contour-step <blocks>] [--contour-major-every <n>] [--contours] [--output <path>]"
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use new_world::world::generation::{
        BoundaryAnchors, BoundaryGuard, BoundaryProfile, MacroFieldContourLevel,
        MacroFieldContourSegment, NoisyBoundaryCurve, VoronoiCornerId, VoronoiEdgeId,
        VoronoiSiteId,
    };

    fn test_config() -> PreviewConfig {
        PreviewConfig {
            seed: 42,
            center_x: -10,
            center_z: 20,
            width: 64,
            height: 32,
            world_span_blocks: 1024,
            region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
            land_bias: 0.14,
            stage: DEFAULT_STAGE.to_string(),
            channel: PreviewChannelSelection::Single(PreviewChannel::LitHeightfield),
            contour_step_blocks: DEFAULT_MACRO_FIELD_CONTOUR_STEP_BLOCKS,
            contour_major_every: DEFAULT_MACRO_FIELD_CONTOUR_MAJOR_EVERY,
            contours: false,
            output: None,
        }
    }

    #[test]
    fn default_output_path_uses_short_seed_center_channel_name() {
        let config = test_config();
        let paths = output_paths_for_config(&config).unwrap();

        assert_eq!(paths.len(), 1);
        assert!(
            paths[0]
                .1
                .display()
                .to_string()
                .replace('\\', "/")
                .ends_with("target/macro-field-preview/s42_x-10_z20_lit.png")
        );
        assert!(!paths[0].1.display().to_string().contains("generator_gv"));
        assert!(!paths[0].1.display().to_string().contains("span"));
    }

    #[test]
    fn all_channel_png_output_becomes_short_directory() {
        let mut config = test_config();
        config.channel = PreviewChannelSelection::All;
        config.output = Some(PathBuf::from("target/macro-field-preview/field-smoke.png"));

        let paths = output_paths_for_config(&config).unwrap();

        assert_eq!(paths.len(), RENDERABLE_CHANNELS.len());
        assert!(paths.iter().any(|(channel, path)| {
            *channel == PreviewChannel::MacroElevation
                && path
                    .display()
                    .to_string()
                    .replace('\\', "/")
                    .ends_with("target/macro-field-preview/field-smoke/macro.png")
        }));
    }

    #[test]
    fn legend_overlay_changes_image_pixels() {
        let mut image = RgbImage::from_pixel(180, 90, image::Rgb([4, 5, 6]));
        let before = image.as_raw().clone();

        draw_legend_overlay(
            &mut image,
            PreviewChannel::CombinedMacroHeight,
            DEFAULT_MACRO_FIELD_CONTOUR_STEP_BLOCKS,
            DEFAULT_MACRO_FIELD_CONTOUR_MAJOR_EVERY,
        );

        assert_ne!(image.as_raw(), &before);
    }

    #[test]
    fn mask_preview_distinguishes_dry_basin_from_coast() {
        let dry = color_for_mask(FieldSample {
            dry_mask: 1.0,
            coast_mask: 1.0,
            ..FieldSample::default()
        });
        let coast = color_for_mask(FieldSample {
            coast_mask: 1.0,
            ..FieldSample::default()
        });

        assert_eq!(dry, MASK_DRY_BASIN_COLOR);
        assert_eq!(coast, MASK_COAST_COLOR);
        assert_ne!(dry, coast);
    }

    #[test]
    fn tile_boundary_overlay_changes_pixels() {
        let mut image = RgbImage::from_pixel(128, 64, image::Rgb([4, 5, 6]));
        let before = image.as_raw().clone();
        let window = PreviewWindow {
            center_x: 0.0,
            center_z: 0.0,
            width: 128,
            height: 64,
            world_span_x: 4096.0,
            world_span_z: 2048.0,
        };
        let grid = tile_grid_stats(window, 1024.0);

        draw_tile_boundary_overlay(&mut image, window, grid);

        assert_ne!(image.as_raw(), &before);
        assert_eq!(grid.vertical_lines, 5);
        assert_eq!(grid.horizontal_lines, 3);
    }

    #[test]
    fn noisy_voronoi_edge_overlay_changes_pixels() {
        let mut image = RgbImage::from_pixel(128, 64, image::Rgb([120, 120, 120]));
        let before = image.as_raw().clone();
        let window = PreviewWindow {
            center_x: 0.0,
            center_z: 0.0,
            width: 128,
            height: 64,
            world_span_x: 1024.0,
            world_span_z: 512.0,
        };
        let boundary = test_boundary_cache();
        let stats = graph_edge_overlay_stats(window, &boundary);

        draw_noisy_graph_edge_overlay(
            &mut image,
            window,
            &boundary,
            PreviewChannel::CombinedMacroHeight,
        );

        assert_ne!(image.as_raw(), &before);
        assert_eq!(stats.noisy_curve_count, 1);
        assert!(stats.drawn_segment_count > 0);
    }

    #[test]
    fn noisy_voronoi_edge_overlay_is_faint_by_default() {
        assert!(
            GRAPH_EDGE_OVERLAY_AMOUNT <= 0.10,
            "macro field graph edge overlay should be a faint reference layer, not a dominant line layer"
        );
    }

    #[test]
    fn lit_voronoi_edge_overlay_is_extra_faint() {
        assert!(
            graph_edge_overlay_amount(PreviewChannel::LitHeightfield)
                < graph_edge_overlay_amount(PreviewChannel::CombinedMacroHeight),
            "lit preview should keep broad hillshade above graph-edge reference lines"
        );
        assert!(
            LIT_GRAPH_EDGE_OVERLAY_AMOUNT <= 0.03,
            "lit graph edge overlay should be almost a reference line"
        );
    }

    #[test]
    fn scale_bar_overlay_changes_pixels_and_reports_length() {
        let mut image = RgbImage::from_pixel(256, 128, image::Rgb([8, 8, 8]));
        let before = image.as_raw().clone();
        let window = PreviewWindow {
            center_x: 0.0,
            center_z: 0.0,
            width: 256,
            height: 128,
            world_span_x: 4096.0,
            world_span_z: 2048.0,
        };
        let stats = scale_bar_stats(window);

        draw_scale_bar_overlay(&mut image, window, stats);

        assert_ne!(image.as_raw(), &before);
        assert!(stats.length_blocks > 0.0);
        assert!(stats.length_pixels > 0);
        assert!(scale_bar_label(stats.length_blocks).contains("BLOCKS"));
    }

    #[test]
    fn height_preview_uses_absolute_scale() {
        assert_eq!(
            normalize_absolute_macro_height(MACRO_PREVIEW_MIN_HEIGHT),
            0.0
        );
        assert_eq!(
            normalize_absolute_macro_height(MACRO_PREVIEW_MAX_HEIGHT),
            1.0
        );
        assert!(
            normalize_absolute_combined_height(0.25) > normalize_absolute_combined_height(0.0),
            "absolute combined preview scale should preserve world height ordering"
        );
        assert!(
            normalize_absolute_combined_height(0.85) < 0.90,
            "ordinary high terrain should not map straight to white"
        );
    }

    #[test]
    fn combined_preview_uses_subtle_terrain_ramp() {
        let low = combined_terrain_ramp(0.08);
        let low_land = combined_terrain_ramp(0.44);
        let high = combined_terrain_ramp(0.92);

        assert!(
            low[2] > low[0] && low[2] > low[1],
            "low combined terrain should read as muted blue-gray, not heat-map black/red: {low:?}"
        );
        assert!(
            low_land[1] >= low_land[0] && low_land[1] >= low_land[2],
            "low land should keep a subdued green-gray terrain bias: {low_land:?}"
        );
        assert!(
            high.iter()
                .all(|channel| *channel < WHITE_SATURATION_THRESHOLD),
            "high combined terrain should be pale without white saturation: {high:?}"
        );
        assert_ne!(
            low, high,
            "combined terrain ramp should still expose macro+ridges-rivers height contrast"
        );
    }

    #[test]
    fn selected_channel_output_has_nonblank_pixels() {
        let tile = MacroFieldTile {
            samples: vec![
                FieldSample {
                    macro_elevation: -1.0,
                    combined_height: -1.0,
                    ..FieldSample::default()
                },
                FieldSample {
                    macro_elevation: 1.0,
                    combined_height: 1.0,
                    ridge_influence: 1.0,
                    ..FieldSample::default()
                },
                FieldSample {
                    macro_elevation: 0.0,
                    combined_height: 0.0,
                    river_valley: 1.0,
                    ..FieldSample::default()
                },
                FieldSample {
                    macro_elevation: 0.5,
                    combined_height: 0.5,
                    coast_mask: 1.0,
                    ..FieldSample::default()
                },
            ],
            macro_stats: ChannelStats {
                min: -1.0,
                max: 1.0,
                average: 0.0,
                robust_min: -1.0,
                robust_max: 1.0,
                contrast_span: 2.0,
            },
            ridge_stats: ChannelStats {
                min: 0.0,
                max: 1.0,
                average: 0.25,
                robust_min: 0.0,
                robust_max: 1.0,
                contrast_span: 1.0,
            },
            river_stats: ChannelStats {
                min: 0.0,
                max: 1.0,
                average: 0.25,
                robust_min: 0.0,
                robust_max: 1.0,
                contrast_span: 1.0,
            },
            combined_stats: ChannelStats {
                min: -1.0,
                max: 1.0,
                average: 0.0,
                robust_min: -1.0,
                robust_max: 1.0,
                contrast_span: 2.0,
            },
            core_stats: CoreMacroFieldTileStats::default(),
            contours: MacroFieldContourSet::default(),
        };
        let window = PreviewWindow {
            center_x: 0.0,
            center_z: 0.0,
            width: 2,
            height: 2,
            world_span_x: 2.0,
            world_span_z: 2.0,
        };

        let image = render_channel(window, &tile, PreviewChannel::RiverValley).unwrap();

        assert!(image.as_raw().iter().any(|channel| *channel != 0));
    }

    #[test]
    fn contour_channel_output_has_nonblank_pixels() {
        let tile = contour_test_tile();
        let window = PreviewWindow {
            center_x: 32.0,
            center_z: 32.0,
            width: 2,
            height: 2,
            world_span_x: 64.0,
            world_span_z: 64.0,
        };

        let image = render_channel(window, &tile, PreviewChannel::Contour).unwrap();

        assert!(image.as_raw().iter().any(|channel| *channel != 0));
    }

    #[test]
    fn contour_overlay_changes_pixels() {
        let tile = contour_test_tile();
        let window = PreviewWindow {
            center_x: 32.0,
            center_z: 32.0,
            width: 128,
            height: 128,
            world_span_x: 64.0,
            world_span_z: 64.0,
        };
        let mut image = RgbImage::from_pixel(128, 128, image::Rgb([24, 28, 31]));
        let before = image.as_raw().clone();

        draw_contour_overlay(&mut image, window, &tile.contours, PreviewChannel::Contour);

        assert_ne!(image.as_raw(), &before);
    }

    #[test]
    fn contour_color_ramp_maps_low_blue_and_high_red() {
        let low = contour_height_color(MACRO_FIELD_CONTOUR_HEIGHT_MIN_BLOCKS, false);
        let high = contour_height_color(MACRO_FIELD_CONTOUR_HEIGHT_MAX_BLOCKS, false);
        let major_high = contour_height_color(MACRO_FIELD_CONTOUR_HEIGHT_MAX_BLOCKS, true);

        assert!(
            low[2] > low[0] && low[2] > low[1],
            "low/oceanward contours should read blue: {low:?}"
        );
        assert!(
            high[0] > high[1] && high[0] > high[2],
            "high contours should read red/orange: {high:?}"
        );
        assert!(
            major_high[0] >= high[0] && major_high[1] >= high[1],
            "major contours should keep the height hue while reading stronger"
        );
    }

    #[test]
    fn contour_preview_options_validate() {
        let config = PreviewConfig {
            channel: PreviewChannelSelection::Single(PreviewChannel::Contour),
            contour_step_blocks: 12.0,
            contour_major_every: 4,
            contours: true,
            ..test_config()
        }
        .validate()
        .unwrap();

        assert_eq!(
            config.channel,
            PreviewChannelSelection::Single(PreviewChannel::Contour)
        );
        assert_eq!(config.contour_step_blocks, 12.0);
        assert_eq!(config.contour_major_every, 4);
        assert!(config.contours);
    }

    #[test]
    fn lit_heightfield_uses_height_gradients() {
        let mut samples = Vec::new();
        for y in 0..3 {
            for x in 0..3 {
                samples.push(FieldSample {
                    combined_height: x as f32 + y as f32 * 0.25,
                    ..FieldSample::default()
                });
            }
        }
        let tile = MacroFieldTile {
            samples,
            combined_stats: ChannelStats {
                min: 0.0,
                max: 2.5,
                average: 1.25,
                robust_min: 0.0,
                robust_max: 2.5,
                contrast_span: 2.5,
            },
            ..MacroFieldTile {
                samples: Vec::new(),
                macro_stats: ChannelStats::default(),
                ridge_stats: ChannelStats::default(),
                river_stats: ChannelStats::default(),
                combined_stats: ChannelStats::default(),
                core_stats: CoreMacroFieldTileStats::default(),
                contours: MacroFieldContourSet::default(),
            }
        };

        let left = lit_height_color(&tile, 0, 1, 3, 3);
        let right = lit_height_color(&tile, 2, 1, 3, 3);

        assert_ne!(
            left, right,
            "lit heightfield should respond to height gradients and height term"
        );
    }

    #[test]
    fn lit_hillshade_keeps_broad_height_contrast_visible() {
        let width = 9;
        let height = 9;
        let mut samples = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let dx = x as f32 / (width - 1) as f32;
                let dy = y as f32 / (height - 1) as f32;
                samples.push(FieldSample {
                    combined_height: -0.35 + dx * 0.85 + dy * 0.25,
                    ..FieldSample::default()
                });
            }
        }
        let tile = MacroFieldTile {
            samples,
            combined_stats: ChannelStats {
                min: -0.35,
                max: 0.75,
                average: 0.20,
                robust_min: -0.35,
                robust_max: 0.75,
                contrast_span: 1.10,
            },
            ..MacroFieldTile {
                samples: Vec::new(),
                macro_stats: ChannelStats::default(),
                ridge_stats: ChannelStats::default(),
                river_stats: ChannelStats::default(),
                combined_stats: ChannelStats::default(),
                core_stats: CoreMacroFieldTileStats::default(),
                contours: MacroFieldContourSet::default(),
            }
        };

        let stats = lit_gradient_stats(&tile, width, height);

        assert!(
            stats.brightness_stddev > 0.04,
            "lit hillshade should keep broad height contrast visible: {stats:?}"
        );
        assert!(
            stats.brightness_max < LIT_MAX_SHADE + f32::EPSILON,
            "lit hillshade should avoid white saturation"
        );
    }

    #[test]
    fn lit_normal_smoothing_reduces_single_sample_spike_gradient() {
        let mut samples = vec![
            FieldSample {
                combined_height: 0.0,
                ..FieldSample::default()
            };
            49
        ];
        samples[3 * 7 + 3].combined_height = 1.0;
        let tile = MacroFieldTile {
            samples,
            combined_stats: ChannelStats {
                min: 0.0,
                max: 1.0,
                average: 1.0 / 49.0,
                robust_min: 0.0,
                robust_max: 1.0,
                contrast_span: 1.0,
            },
            ..MacroFieldTile {
                samples: Vec::new(),
                macro_stats: ChannelStats::default(),
                ridge_stats: ChannelStats::default(),
                river_stats: ChannelStats::default(),
                combined_stats: ChannelStats::default(),
                core_stats: CoreMacroFieldTileStats::default(),
                contours: MacroFieldContourSet::default(),
            }
        };
        let raw = raw_height_gradient(&tile, 2, 3, 7, 7);
        let smoothed = smoothed_lit_gradient(&tile, 2, 3, 7, 7);

        assert!(
            smoothed < raw,
            "lit normal should smooth preview-only spikes: raw={raw} smoothed={smoothed}"
        );
    }

    #[test]
    fn lit_heightfield_avoids_full_white_saturation() {
        let samples = vec![
            FieldSample {
                combined_height: 0.85,
                ..FieldSample::default()
            };
            9
        ];
        let tile = MacroFieldTile {
            samples,
            combined_stats: ChannelStats {
                min: 0.85,
                max: 0.85,
                average: 0.85,
                robust_min: 0.85,
                robust_max: 0.85,
                contrast_span: 0.0,
            },
            ..MacroFieldTile {
                samples: Vec::new(),
                macro_stats: ChannelStats::default(),
                ridge_stats: ChannelStats::default(),
                river_stats: ChannelStats::default(),
                combined_stats: ChannelStats::default(),
                core_stats: CoreMacroFieldTileStats::default(),
                contours: MacroFieldContourSet::default(),
            }
        };

        let color = lit_height_color(&tile, 1, 1, 3, 3);

        assert!(
            color
                .iter()
                .any(|channel| *channel < WHITE_SATURATION_THRESHOLD),
            "lit preview should not turn normal high terrain into saturated white: {color:?}"
        );
    }

    fn contour_test_tile() -> MacroFieldTile {
        MacroFieldTile {
            samples: vec![
                FieldSample {
                    combined_height: -0.2,
                    ..FieldSample::default()
                },
                FieldSample {
                    combined_height: 0.4,
                    ..FieldSample::default()
                },
                FieldSample {
                    combined_height: -0.1,
                    ..FieldSample::default()
                },
                FieldSample {
                    combined_height: 0.5,
                    ..FieldSample::default()
                },
            ],
            combined_stats: ChannelStats {
                min: -0.2,
                max: 0.5,
                average: 0.15,
                robust_min: -0.2,
                robust_max: 0.5,
                contrast_span: 0.7,
            },
            contours: MacroFieldContourSet {
                step_blocks: 8.0,
                major_every: 5,
                min_level_blocks: 0.0,
                max_level_blocks: 0.0,
                total_segment_count: 1,
                levels: vec![MacroFieldContourLevel {
                    height_blocks: 0.0,
                    is_major: true,
                    segments: vec![MacroFieldContourSegment {
                        start: WorldPlanePoint::new(16.0, 0.0),
                        end: WorldPlanePoint::new(16.0, 64.0),
                    }],
                }],
            },
            ..MacroFieldTile {
                samples: Vec::new(),
                macro_stats: ChannelStats::default(),
                ridge_stats: ChannelStats::default(),
                river_stats: ChannelStats::default(),
                combined_stats: ChannelStats::default(),
                core_stats: CoreMacroFieldTileStats::default(),
                contours: MacroFieldContourSet::default(),
            }
        }
    }

    fn test_boundary_cache() -> BoundaryCache {
        BoundaryCache {
            curves: vec![NoisyBoundaryCurve {
                edge: VoronoiEdgeId(1),
                profile: BoundaryProfile::Ordinary,
                anchors: BoundaryAnchors {
                    corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                    sites: [VoronoiSiteId(1), VoronoiSiteId(2)],
                    start: WorldPlanePoint::new(-300.0, -100.0),
                    end: WorldPlanePoint::new(300.0, 100.0),
                },
                points: vec![
                    WorldPlanePoint::new(-300.0, -100.0),
                    WorldPlanePoint::new(-120.0, 40.0),
                    WorldPlanePoint::new(80.0, -30.0),
                    WorldPlanePoint::new(300.0, 100.0),
                ],
                amplitude: 32.0,
                seed: 1,
                guard: BoundaryGuard {
                    min_x: -512.0,
                    max_x: 512.0,
                    min_z: -256.0,
                    max_z: 256.0,
                },
            }],
            stats: Default::default(),
        }
    }
}
