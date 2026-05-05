use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{self, BufWriter, ErrorKind};
use std::path::{Path, PathBuf};

use image::RgbImage;
use rayon::prelude::*;

use new_world::world::WorldMeta;
use new_world::world::generation::{
    BoundaryCache, BoundaryConfig, DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS,
    GraphHydrologyGraph, GraphMacroMap, GraphRegionArea, GraphRegionCoord, GraphRiverSegment,
    HydrologyConfig, MacroFieldSample as CoreMacroFieldSample,
    MacroFieldTileConfig as CoreMacroFieldTileConfig, MacroMapConfig, MacroSite, MacroSurfaceKind,
    NoisyBoundaryCurve, VoronoiGraphConfig, VoronoiGraphPatch, VoronoiGraphPatchRequest,
    VoronoiSiteId, WorldPlanePoint, generate_macro_field_tile, generate_macro_map,
    generate_noisy_boundaries, generate_voronoi_graph_patch, graph_region_for_world_block,
    solve_hydrology,
};

const DEFAULT_WIDTH: u32 = 3840;
const DEFAULT_HEIGHT: u32 = 2160;
const DEFAULT_WORLD_SPAN_BLOCKS: i32 = 32768;
const DEFAULT_STAGE: &str = "macro_field";
const OUTPUT_DIR: &str = "target/macro-field-preview";
const FEATURE_BUCKET_BLOCKS: f32 = 256.0;
const RIDGE_RADIUS_BLOCKS: f32 = 760.0;
const RIVER_RADIUS_BLOCKS: f32 = 420.0;
const COAST_RADIUS_BLOCKS: f32 = 520.0;

const RENDERABLE_CHANNELS: [PreviewChannel; 6] = [
    PreviewChannel::MacroElevation,
    PreviewChannel::Mask,
    PreviewChannel::RidgeInfluence,
    PreviewChannel::RiverValley,
    PreviewChannel::CombinedMacroHeight,
    PreviewChannel::LitHeightfield,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SiteGridCoord {
    x: i32,
    z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FeatureGridCoord {
    x: i32,
    z: i32,
}

#[derive(Debug, Clone)]
struct PreviewWorld {
    patch: VoronoiGraphPatch,
    macro_map: GraphMacroMap,
    hydrology: GraphHydrologyGraph,
    boundary: BoundaryCache,
    site_grid: HashMap<SiteGridCoord, usize>,
    site_samples: HashMap<VoronoiSiteId, MacroSite>,
    ridge_grid: FeatureGrid,
    river_grid: FeatureGrid,
    coast_grid: FeatureGrid,
    river_segment_count: usize,
    spacing: f32,
}

#[derive(Debug, Clone, Copy)]
struct NearestSite {
    index: usize,
}

#[derive(Debug, Clone, Copy)]
struct FeatureSamplePoint {
    position: WorldPlanePoint,
    strength: f32,
}

#[derive(Debug, Clone, Default)]
struct FeatureGrid {
    buckets: HashMap<FeatureGridCoord, Vec<FeatureSamplePoint>>,
}

impl FeatureGrid {
    fn insert(&mut self, point: FeatureSamplePoint) {
        self.buckets
            .entry(feature_grid_coord(point.position))
            .or_default()
            .push(point);
    }

    fn influence_at(&self, position: WorldPlanePoint, radius: f32) -> f32 {
        if radius <= 0.0 || self.buckets.is_empty() {
            return 0.0;
        }
        let center = feature_grid_coord(position);
        let search = (radius / FEATURE_BUCKET_BLOCKS).ceil() as i32 + 1;
        let radius_sq = radius * radius;
        let mut influence = 0.0_f32;

        for oz in -search..=search {
            for ox in -search..=search {
                let Some(points) = self.buckets.get(&FeatureGridCoord {
                    x: center.x + ox,
                    z: center.z + oz,
                }) else {
                    continue;
                };
                for point in points {
                    let dx = point.position.x - position.x;
                    let dz = point.position.z - position.z;
                    let distance_sq = dx * dx + dz * dz;
                    if distance_sq > radius_sq {
                        continue;
                    }
                    let t = 1.0 - distance_sq.sqrt() / radius;
                    influence = influence.max(smoothstep01(t) * point.strength);
                }
            }
        }

        influence.clamp(0.0, 1.0)
    }
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

#[derive(Debug, Clone)]
struct MacroFieldTile {
    samples: Vec<FieldSample>,
    macro_stats: ChannelStats,
    ridge_stats: ChannelStats,
    river_stats: ChannelStats,
    combined_stats: ChannelStats,
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
    macro_stats: ChannelStats,
    ridge_stats: ChannelStats,
    river_stats: ChannelStats,
    combined_stats: ChannelStats,
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
            "lit=topdown_white_heightfield_shaded_from_combined_height_gradient".to_string(),
        ]
        .join("\n")
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?.validate()?;
    let meta = WorldMeta::new(config.seed);
    let window = config.window();
    let graph_area = window.graph_area(config.region_size_blocks)?;
    let preview = build_preview_world(&meta, &config, graph_area)?;
    let tile = rasterize_macro_field(window, &preview)?;
    let output_paths = output_paths_for_config(&config)?;
    let mut generated = Vec::with_capacity(output_paths.len());

    for (channel, output) in output_paths {
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
            ridge_feature_samples: preview.ridge_grid.sample_count(),
            river_feature_samples: preview.river_grid.sample_count(),
            coast_feature_samples: preview.coast_grid.sample_count(),
            macro_stats: tile.macro_stats,
            ridge_stats: tile.ridge_stats,
            river_stats: tile.river_stats,
            combined_stats: tile.combined_stats,
        };
        let mut image = render_channel(window, &tile, channel)?;
        draw_legend_overlay(&mut image, channel);
        write_png_with_metadata(&image, &output, &header)?;
        generated.push((channel, output, image.width(), image.height()));
    }

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
        "stage inputs: sites {}, macro edges {}, boundary curves {}, selected river segments {}, river feature samples {}",
        preview.patch.sites.len(),
        preview.macro_map.edges.len(),
        preview.boundary.curves.len(),
        preview.river_segment_count,
        preview.river_grid.sample_count()
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
        "river valley min/avg/max {:.3}/{:.3}/{:.3}",
        tile.river_stats.min, tile.river_stats.average, tile.river_stats.max
    );
    println!(
        "combined height min/avg/max {:.3}/{:.3}/{:.3}",
        tile.combined_stats.min, tile.combined_stats.average, tile.combined_stats.max
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

impl FeatureGrid {
    fn sample_count(&self) -> usize {
        self.buckets.values().map(Vec::len).sum()
    }
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
    let spacing = config.site_spacing_blocks as f32;
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
    let site_grid = patch
        .sites
        .iter()
        .enumerate()
        .map(|(index, site)| (site_grid_coord_for_position(site.position, spacing), index))
        .collect::<HashMap<_, _>>();
    if site_grid.is_empty() {
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
    let site_samples = macro_map
        .sites
        .iter()
        .map(|site| (site.id, *site))
        .collect::<HashMap<_, _>>();
    let edge_samples = macro_map
        .edges
        .iter()
        .map(|edge| (edge.id, *edge))
        .collect::<HashMap<_, _>>();
    let mut ridge_grid = FeatureGrid::default();
    let mut river_grid = FeatureGrid::default();
    let mut coast_grid = FeatureGrid::default();

    for edge in &macro_map.edges {
        let Some(curve) = boundary.curve_for_edge(edge.id) else {
            continue;
        };
        if edge.guide.is_ridge_candidate {
            insert_curve_samples(
                &mut ridge_grid,
                curve,
                (0.35 + edge.guide.ridgeness * 0.65).clamp(0.0, 1.0),
            );
        }
        if edge.guide.is_coast {
            insert_curve_samples(
                &mut coast_grid,
                curve,
                (0.35 + edge.guide.coastness * 0.65).clamp(0.0, 1.0),
            );
        }
    }

    for segment in &hydrology.segments {
        if edge_samples
            .get(&segment.edge)
            .is_some_and(|edge| edge.lake_class.excludes_selected_river())
        {
            continue;
        }
        let Some(curve) = boundary.curve_for_edge(segment.edge) else {
            continue;
        };
        insert_curve_samples(&mut river_grid, curve, river_strength(segment));
    }

    let river_segment_count = hydrology.segments.len();

    Ok(PreviewWorld {
        patch,
        macro_map,
        hydrology,
        boundary,
        site_grid,
        site_samples,
        ridge_grid,
        river_grid,
        coast_grid,
        river_segment_count,
        spacing,
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

fn site_grid_coord_for_position(position: WorldPlanePoint, spacing: f32) -> SiteGridCoord {
    SiteGridCoord {
        x: (position.x / spacing).floor() as i32,
        z: (position.z / spacing).floor() as i32,
    }
}

fn feature_grid_coord(position: WorldPlanePoint) -> FeatureGridCoord {
    FeatureGridCoord {
        x: (position.x / FEATURE_BUCKET_BLOCKS).floor() as i32,
        z: (position.z / FEATURE_BUCKET_BLOCKS).floor() as i32,
    }
}

fn insert_curve_samples(grid: &mut FeatureGrid, curve: &NoisyBoundaryCurve, strength: f32) {
    for point in &curve.points {
        grid.insert(FeatureSamplePoint {
            position: *point,
            strength,
        });
    }
}

fn river_strength(segment: &GraphRiverSegment) -> f32 {
    ((segment.flow_accumulation.max(0.0) + 1.0).ln() / 7.0).clamp(0.15, 1.0)
}

fn rasterize_macro_field(
    window: PreviewWindow,
    preview: &PreviewWorld,
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

#[allow(dead_code)]
fn sample_field(preview: &PreviewWorld, position: WorldPlanePoint) -> FieldSample {
    let site = nearest_site(preview, position.x, position.z);
    let macro_site = preview
        .site_samples
        .get(&preview.patch.sites[site.index].id)
        .copied()
        .expect("macro map should contain every graph site");
    let ridge = preview
        .ridge_grid
        .influence_at(position, RIDGE_RADIUS_BLOCKS)
        .max(macro_site.ridgeness * 0.35);
    let river = preview
        .river_grid
        .influence_at(position, RIVER_RADIUS_BLOCKS);
    let coast = preview
        .coast_grid
        .influence_at(position, COAST_RADIUS_BLOCKS)
        .max(macro_site.coastness * 0.7)
        .clamp(0.0, 1.0);
    let ocean = if macro_site.surface_kind.is_ocean_owned() {
        1.0
    } else {
        0.0
    };
    let lake = if matches!(
        macro_site.surface_kind,
        MacroSurfaceKind::LakeCandidate | MacroSurfaceKind::WetlandCandidate
    ) {
        1.0
    } else {
        0.0
    };
    let dry = if macro_site.surface_kind == MacroSurfaceKind::DryBasin {
        1.0
    } else {
        0.0
    };

    let water_flatten = (ocean * 0.55_f32 + lake * 0.42_f32).clamp(0.0, 0.65);
    let coast_flatten = coast * 0.10;
    let ridge_raise = ridge * (0.20 + macro_site.mountainness.max(0.0) * 0.36);
    let river_carve = river * (0.10 + 0.26 * (1.0 - lake * 0.5));
    let combined_height = (macro_site.signed_macro_elevation + ridge_raise
        - river_carve
        - coast_flatten
        - water_flatten)
        .clamp(-1.25, 1.75);

    FieldSample {
        macro_elevation: macro_site.signed_macro_elevation,
        ocean_mask: ocean,
        lake_mask: lake,
        coast_mask: coast,
        dry_mask: dry,
        ridge_influence: ridge,
        river_valley: river,
        combined_height,
    }
}

fn nearest_site(preview: &PreviewWorld, world_x: f32, world_z: f32) -> NearestSite {
    let center_cell_x = (world_x / preview.spacing).floor() as i32;
    let center_cell_z = (world_z / preview.spacing).floor() as i32;
    let mut nearest_index = 0;
    let mut nearest_distance_sq = f32::MAX;

    for offset_z in -2..=2 {
        for offset_x in -2..=2 {
            let coord = SiteGridCoord {
                x: center_cell_x + offset_x,
                z: center_cell_z + offset_z,
            };
            let Some(index) = preview.site_grid.get(&coord).copied() else {
                continue;
            };
            let site = preview.patch.sites[index];
            let dx = site.position.x - world_x;
            let dz = site.position.z - world_z;
            let distance_sq = dx * dx + dz * dz;
            if distance_sq < nearest_distance_sq {
                nearest_distance_sq = distance_sq;
                nearest_index = index;
            }
        }
    }

    NearestSite {
        index: nearest_index,
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
            gradient_macro(normalize_contrast(sample.macro_elevation, tile.macro_stats))
        }
        PreviewChannel::Mask => color_for_mask(sample),
        PreviewChannel::RidgeInfluence => gradient_fire(sample.ridge_influence),
        PreviewChannel::RiverValley => gradient_river(sample.river_valley),
        PreviewChannel::CombinedMacroHeight => gradient_height(normalize_contrast(
            sample.combined_height,
            tile.combined_stats,
        )),
        PreviewChannel::LitHeightfield => lit_height_color(tile, x, y, width, height),
    }
}

fn color_for_mask(sample: FieldSample) -> [u8; 3] {
    if sample.ocean_mask > 0.5 {
        return [31, 90, 164];
    }
    if sample.lake_mask > 0.5 {
        return [54, 150, 198];
    }
    if sample.dry_mask > 0.5 {
        return [143, 137, 83];
    }
    if sample.coast_mask > 0.45 {
        return [220, 196, 125];
    }
    [101, 154, 89]
}

fn lit_height_color(
    tile: &MacroFieldTile,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) -> [u8; 3] {
    let left = tile.samples[y * width + x.saturating_sub(1)].combined_height;
    let right = tile.samples[y * width + (x + 1).min(width - 1)].combined_height;
    let up = tile.samples[y.saturating_sub(1) * width + x].combined_height;
    let down = tile.samples[(y + 1).min(height - 1) * width + x].combined_height;
    let dx = right - left;
    let dz = down - up;
    let normal = normalize3([-dx * 5.0, 1.0, -dz * 5.0]);
    let light = normalize3([-0.45, 0.78, -0.43]);
    let diffuse = dot3(normal, light).max(0.0);
    let height_t = normalize_contrast(
        tile.samples[y * width + x].combined_height,
        tile.combined_stats,
    );
    let ambient = 0.34 + height_t * 0.20;
    let shade = (ambient + diffuse * 0.70).clamp(0.0, 1.0);
    let value = (shade * 255.0).round() as u8;
    [value, value, value]
}

fn normalize(value: f32, stats: ChannelStats) -> f32 {
    let span = stats.max - stats.min;
    if span.abs() <= f32::EPSILON {
        0.5
    } else {
        ((value - stats.min) / span).clamp(0.0, 1.0)
    }
}

fn normalize_contrast(value: f32, stats: ChannelStats) -> f32 {
    let span = stats.robust_max - stats.robust_min;
    let base = if span.abs() <= f32::EPSILON {
        normalize(value, stats)
    } else {
        ((value - stats.robust_min) / span).clamp(0.0, 1.0)
    };
    ((base - 0.5) * 1.22 + 0.5).clamp(0.0, 1.0).powf(0.92)
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

fn gradient_height(value: f32) -> [u8; 3] {
    gradient_color(
        value,
        &[
            (0.00, [38, 78, 119]),
            (0.35, [80, 139, 102]),
            (0.58, [168, 158, 96]),
            (0.80, [127, 119, 112]),
            (1.00, [245, 246, 240]),
        ],
    )
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

fn draw_legend_overlay(image: &mut RgbImage, channel: PreviewChannel) {
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
    let panel_height = (50 * scale).min(image.height());
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
                    if t < 0.25 {
                        [31, 90, 164]
                    } else if t < 0.50 {
                        [54, 150, 198]
                    } else if t < 0.75 {
                        [220, 196, 125]
                    } else {
                        [101, 154, 89]
                    }
                }
                PreviewChannel::RidgeInfluence => gradient_fire(t),
                PreviewChannel::RiverValley => gradient_river(t),
                PreviewChannel::CombinedMacroHeight => gradient_height(t),
                PreviewChannel::LitHeightfield => {
                    let v = (t * 255.0).round() as u8;
                    [v, v, v]
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

fn mix_channel(base: u8, tint: u8, amount: f32) -> u8 {
    ((base as f32 * (1.0 - amount)) + (tint as f32 * amount))
        .round()
        .clamp(0.0, 255.0) as u8
}

fn smoothstep01(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
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
    "usage: cargo run --bin macro_field_preview -- <seed> <center-x> <center-z> [--width <u32>] [--height <u32>] [--world-span-blocks <i32>] [--region-size-blocks <i32>] [--site-spacing-blocks <i32>] [--land-bias <f32>] [--stage macro_field] [--channel <all|macro|mask|ridge|river|combined|lit>] [--output <path>]"
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

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

        draw_legend_overlay(&mut image, PreviewChannel::CombinedMacroHeight);

        assert_ne!(image.as_raw(), &before);
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
            }
        };

        let left = lit_height_color(&tile, 0, 1, 3, 3);
        let right = lit_height_color(&tile, 2, 1, 3, 3);

        assert_ne!(
            left, right,
            "lit heightfield should respond to height gradients and height term"
        );
    }
}
