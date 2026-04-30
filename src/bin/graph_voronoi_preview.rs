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
    DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS, GraphRegionArea,
    GraphRegionCoord, VoronoiGraphConfig, VoronoiGraphPatch, VoronoiGraphPatchRequest, VoronoiSite,
    WorldPlanePoint, generate_voronoi_graph_patch, graph_region_for_world_block,
};

const DEFAULT_WIDTH: u32 = 3840;
const DEFAULT_HEIGHT: u32 = 2160;
const DEFAULT_WORLD_SPAN_BLOCKS: i32 = 8192;
const DEFAULT_STAGE: &str = "graph_voronoi";
const OUTPUT_DIR: &str = "target/graph-voronoi-preview";

const RENDERABLE_MODES: [PreviewMode; 6] = [
    PreviewMode::Identity,
    PreviewMode::Temperature,
    PreviewMode::Hydration,
    PreviewMode::Continentality,
    PreviewMode::Elevation,
    PreviewMode::Ruggedness,
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
    stage: String,
    mode: PreviewModeSelection,
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

    fn output_path(&self, generator_version: u32) -> PathBuf {
        self.output.clone().unwrap_or_else(|| {
            PathBuf::from(format!(
                "{OUTPUT_DIR}/seed_{}_cx{}_cz{}_generator_gv{}_stage_{}_{}x{}_span{}_spacing{}.png",
                self.seed,
                self.center_x,
                self.center_z,
                generator_version,
                self.stage,
                self.width,
                self.height,
                self.world_span_blocks,
                self.site_spacing_blocks
            ))
        })
    }

    fn output_dir(&self, generator_version: u32) -> PathBuf {
        self.output.clone().unwrap_or_else(|| {
            PathBuf::from(format!(
                "{OUTPUT_DIR}/seed_{}_cx{}_cz{}_generator_gv{}_stage_{}_{}x{}_span{}_spacing{}",
                self.seed,
                self.center_x,
                self.center_z,
                generator_version,
                self.stage,
                self.width,
                self.height,
                self.world_span_blocks,
                self.site_spacing_blocks
            ))
        })
    }

    fn default_mode_file_name(&self, generator_version: u32, mode: PreviewMode) -> String {
        format!(
            "seed_{}_cx{}_cz{}_generator_gv{}_stage_{}_mode_{}_{}x{}_span{}_spacing{}.png",
            self.seed,
            self.center_x,
            self.center_z,
            generator_version,
            self.stage,
            mode.as_str(),
            self.width,
            self.height,
            self.world_span_blocks,
            self.site_spacing_blocks
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewModeSelection {
    All,
    Single(PreviewMode),
}

impl PreviewModeSelection {
    fn modes(self) -> &'static [PreviewMode] {
        match self {
            PreviewModeSelection::All => &RENDERABLE_MODES,
            PreviewModeSelection::Single(PreviewMode::Identity) => &RENDERABLE_MODES[0..1],
            PreviewModeSelection::Single(PreviewMode::Temperature) => &RENDERABLE_MODES[1..2],
            PreviewModeSelection::Single(PreviewMode::Hydration) => &RENDERABLE_MODES[2..3],
            PreviewModeSelection::Single(PreviewMode::Continentality) => &RENDERABLE_MODES[3..4],
            PreviewModeSelection::Single(PreviewMode::Elevation) => &RENDERABLE_MODES[4..5],
            PreviewModeSelection::Single(PreviewMode::Ruggedness) => &RENDERABLE_MODES[5..6],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewMode {
    Identity,
    Temperature,
    Hydration,
    Continentality,
    Elevation,
    Ruggedness,
}

impl PreviewMode {
    fn parse(value: &str) -> Option<PreviewModeSelection> {
        match value {
            "all" => Some(PreviewModeSelection::All),
            "identity" => Some(PreviewModeSelection::Single(Self::Identity)),
            "temperature" => Some(PreviewModeSelection::Single(Self::Temperature)),
            "hydration" | "humidity" => Some(PreviewModeSelection::Single(Self::Hydration)),
            "continentality" => Some(PreviewModeSelection::Single(Self::Continentality)),
            "elevation" => Some(PreviewModeSelection::Single(Self::Elevation)),
            "ruggedness" => Some(PreviewModeSelection::Single(Self::Ruggedness)),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Temperature => "temperature",
            Self::Hydration => "hydration",
            Self::Continentality => "continentality",
            Self::Elevation => "elevation",
            Self::Ruggedness => "ruggedness",
        }
    }

    fn map_name(self) -> &'static str {
        match self {
            Self::Identity => "graph site identity",
            Self::Temperature => "site temperature",
            Self::Hydration => "site hydration",
            Self::Continentality => "site continentality",
            Self::Elevation => "site elevation bias",
            Self::Ruggedness => "site ruggedness",
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

    fn pixel_span(self) -> f32 {
        self.world_span_x / self.width as f32
    }

    fn sample_world_x(self, pixel_x: u32) -> f32 {
        self.min_x() + (pixel_x as f32 + 0.5) * self.world_span_x / self.width as f32
    }

    fn sample_world_z(self, pixel_z: u32) -> f32 {
        self.min_z() + (pixel_z as f32 + 0.5) * self.world_span_z / self.height as f32
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
        GraphRegionArea::new(min, max).ok_or_else(|| cli_error("invalid graph preview area"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SiteGridCoord {
    x: i32,
    z: i32,
}

#[derive(Debug, Clone)]
struct PreviewGraph {
    patch: VoronoiGraphPatch,
    site_grid: HashMap<SiteGridCoord, usize>,
    spacing: f32,
}

#[derive(Debug, Clone, Copy)]
struct NearestSites {
    nearest_index: usize,
    nearest_distance_sq: f32,
    second_distance_sq: f32,
}

#[derive(Debug, Clone)]
struct PreviewHeader {
    binary: &'static str,
    seed: u64,
    generator_version: u32,
    stage: String,
    mode: PreviewMode,
    center_x: i32,
    center_z: i32,
    width: u32,
    height: u32,
    world_span_blocks: i32,
    region_size_blocks: i32,
    site_spacing_blocks: i32,
    graph_area: GraphRegionArea,
    site_count: usize,
    owner_region_count: usize,
    graph_source: &'static str,
}

impl PreviewHeader {
    fn to_metadata_text(&self) -> String {
        [
            format!("binary={}", self.binary),
            format!("seed={}", self.seed),
            format!("generator_version={}", self.generator_version),
            format!("stage={}", self.stage),
            format!("mode={}", self.mode.as_str()),
            format!("map_name={}", self.mode.map_name()),
            format!("center_x={}", self.center_x),
            format!("center_z={}", self.center_z),
            format!("width={}", self.width),
            format!("height={}", self.height),
            format!("world_span_blocks={}", self.world_span_blocks),
            format!("region_size_blocks={}", self.region_size_blocks),
            format!("site_spacing_blocks={}", self.site_spacing_blocks),
            format!(
                "graph_area_min={},{}",
                self.graph_area.min.x, self.graph_area.min.z
            ),
            format!(
                "graph_area_max={},{}",
                self.graph_area.max.x, self.graph_area.max.z
            ),
            format!("site_count={}", self.site_count),
            format!("owner_region_count={}", self.owner_region_count),
            format!("graph_source={}", self.graph_source),
        ]
        .join("\n")
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?.validate()?;
    let meta = WorldMeta::new(config.seed);
    let window = config.window();
    let graph_area = window.graph_area(config.region_size_blocks)?;
    let graph = build_graph_patch_for_preview(&meta, &config, graph_area)?;

    let output_paths = output_paths_for_config(&config, meta.generator_version)?;
    let mut generated = Vec::with_capacity(output_paths.len());
    for (mode, output) in output_paths {
        let header = PreviewHeader {
            binary: "graph_voronoi_preview",
            seed: config.seed,
            generator_version: meta.generator_version,
            stage: config.stage.clone(),
            mode,
            center_x: config.center_x,
            center_z: config.center_z,
            width: config.width,
            height: config.height,
            world_span_blocks: config.world_span_blocks,
            region_size_blocks: config.region_size_blocks,
            site_spacing_blocks: config.site_spacing_blocks,
            graph_area,
            site_count: graph.patch.sites.len(),
            owner_region_count: graph.patch.owner_regions.len(),
            graph_source: "world_generation_graph",
        };

        let image = render_preview(window, &graph, config.region_size_blocks, mode)?;
        write_png_with_metadata(&image, &output, &header)?;
        generated.push((mode, output, image.width(), image.height()));
    }

    println!("seed: {}", config.seed);
    println!("generator version: {}", meta.generator_version);
    println!("stage: {}", config.stage);
    println!(
        "modes: {}",
        config
            .mode
            .modes()
            .iter()
            .map(|mode| mode.as_str())
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
        "graph regions: x={}..{}, z={}..{} ({} owned regions)",
        graph_area.min.x,
        graph_area.max.x,
        graph_area.min.z,
        graph_area.max.z,
        graph.patch.owner_regions.len()
    );
    println!(
        "site spacing: {} blocks, sites: {}",
        config.site_spacing_blocks,
        graph.patch.sites.len()
    );
    println!("metadata: new-world-preview-header iTXt chunk");
    println!();
    println!("generated files:");
    for (mode, output, width, height) in generated {
        println!(
            "{} {}x{} {}",
            mode.as_str(),
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
    let mut stage = DEFAULT_STAGE.to_string();
    let mut mode = PreviewModeSelection::Single(PreviewMode::Identity);
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
            "--stage" => stage = parse_required::<String>(&mut args, "stage")?,
            "--mode" => {
                let value = parse_required::<String>(&mut args, "mode")?;
                mode = PreviewMode::parse(&value).ok_or_else(|| {
                    cli_error(format!("unsupported mode: {value}\n\n{}", usage()))
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
        stage,
        mode,
        output,
    })
}

fn output_paths_for_config(
    config: &PreviewConfig,
    generator_version: u32,
) -> Result<Vec<(PreviewMode, PathBuf)>, Box<dyn Error>> {
    let modes = config.mode.modes();
    if matches!(config.mode, PreviewModeSelection::All) {
        if config
            .output
            .as_ref()
            .is_some_and(|path| looks_like_file(path))
        {
            return Err(cli_error(
                "--mode all requires --output to be a directory path, not a PNG file",
            ));
        }
        let output_dir = config.output_dir(generator_version);
        return Ok(modes
            .iter()
            .copied()
            .map(|mode| {
                (
                    mode,
                    output_dir.join(config.default_mode_file_name(generator_version, mode)),
                )
            })
            .collect());
    }

    let mode = modes[0];
    let output = config.output.as_ref().map_or_else(
        || {
            if mode == PreviewMode::Identity {
                config.output_path(generator_version)
            } else {
                PathBuf::from(OUTPUT_DIR)
                    .join(config.default_mode_file_name(generator_version, mode))
            }
        },
        |path| {
            if looks_like_file(path) {
                path.clone()
            } else {
                path.join(config.default_mode_file_name(generator_version, mode))
            }
        },
    );

    Ok(vec![(mode, output)])
}

fn looks_like_file(path: &Path) -> bool {
    path.extension().is_some()
}

fn build_graph_patch_for_preview(
    meta: &WorldMeta,
    config: &PreviewConfig,
    graph_area: GraphRegionArea,
) -> Result<PreviewGraph, Box<dyn Error>> {
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

    Ok(PreviewGraph {
        patch,
        site_grid,
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
        .map_err(|_| cli_error("graph preview padding overflowed"))
}

fn site_grid_coord_for_position(position: WorldPlanePoint, spacing: f32) -> SiteGridCoord {
    SiteGridCoord {
        x: (position.x / spacing).floor() as i32,
        z: (position.z / spacing).floor() as i32,
    }
}

fn render_preview(
    window: PreviewWindow,
    graph: &PreviewGraph,
    region_size_blocks: i32,
    mode: PreviewMode,
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
            let pixel_x = (index as u32) % window.width;
            let pixel_z = (index as u32) / window.width;
            let world_x = window.sample_world_x(pixel_x);
            let world_z = window.sample_world_z(pixel_z);
            let color =
                color_for_world_sample(window, graph, region_size_blocks, mode, world_x, world_z);
            pixel.copy_from_slice(&color);
        });

    RgbImage::from_raw(window.width, window.height, pixels)
        .ok_or_else(|| cli_error("failed to build RGB image buffer"))
}

fn color_for_world_sample(
    window: PreviewWindow,
    graph: &PreviewGraph,
    region_size_blocks: i32,
    mode: PreviewMode,
    world_x: f32,
    world_z: f32,
) -> [u8; 3] {
    let nearest = nearest_sites(graph, world_x, world_z);
    let site = graph.patch.sites[nearest.nearest_index];
    let mut color = color_for_site(site, mode);

    let nearest_distance = nearest.nearest_distance_sq.sqrt();
    let second_distance = nearest.second_distance_sq.sqrt();
    let edge_strength =
        (1.0 - ((second_distance - nearest_distance) / (graph.spacing * 0.10))).clamp(0.0, 1.0);
    if edge_strength > 0.0 {
        color = blend(
            color,
            [20, 24, 34],
            edge_strength * edge_overlay_amount(mode),
        );
    }

    let dot_radius = (graph.spacing * 0.032).max(window.pixel_span() * 1.25);
    if nearest_distance <= dot_radius {
        let dot = blend([246, 248, 240], color_for_site(site, mode), 0.28);
        color = blend(color, dot, 0.88);
    }

    let region_line =
        region_grid_strength(world_x, world_z, region_size_blocks, window.pixel_span());
    if region_line > 0.0 {
        color = blend(color, [238, 200, 98], region_line * 0.42);
    }

    color
}

fn nearest_sites(graph: &PreviewGraph, world_x: f32, world_z: f32) -> NearestSites {
    let center_cell_x = (world_x / graph.spacing).floor() as i32;
    let center_cell_z = (world_z / graph.spacing).floor() as i32;
    let mut nearest_index = 0;
    let mut nearest_distance_sq = f32::MAX;
    let mut second_distance_sq = f32::MAX;

    for offset_z in -2..=2 {
        for offset_x in -2..=2 {
            let coord = SiteGridCoord {
                x: center_cell_x + offset_x,
                z: center_cell_z + offset_z,
            };
            let Some(index) = graph.site_grid.get(&coord).copied() else {
                continue;
            };
            let site = graph.patch.sites[index];
            let dx = site.position.x - world_x;
            let dz = site.position.z - world_z;
            let distance_sq = dx * dx + dz * dz;
            if distance_sq < nearest_distance_sq {
                second_distance_sq = nearest_distance_sq;
                nearest_distance_sq = distance_sq;
                nearest_index = index;
            } else if distance_sq < second_distance_sq {
                second_distance_sq = distance_sq;
            }
        }
    }

    if second_distance_sq == f32::MAX {
        second_distance_sq = nearest_distance_sq;
    }

    NearestSites {
        nearest_index,
        nearest_distance_sq,
        second_distance_sq,
    }
}

fn color_for_site(site: VoronoiSite, mode: PreviewMode) -> [u8; 3] {
    match mode {
        PreviewMode::Identity => color_for_identity_site(site),
        PreviewMode::Temperature => gradient_color(
            site.base_fields.temperature.clamp(0.0, 1.0),
            &[
                (0.00, [20, 42, 116]),
                (0.38, [82, 161, 213]),
                (0.55, [230, 232, 194]),
                (0.75, [220, 126, 68]),
                (1.00, [164, 37, 43]),
            ],
        ),
        PreviewMode::Hydration => gradient_color(
            site.base_fields.hydration.clamp(0.0, 1.0),
            &[
                (0.00, [173, 119, 55]),
                (0.35, [218, 190, 108]),
                (0.62, [92, 158, 104]),
                (1.00, [40, 118, 157]),
            ],
        ),
        PreviewMode::Continentality => gradient_color(
            signed_to_unit(site.base_fields.continentality),
            &[
                (0.00, [24, 80, 146]),
                (0.42, [83, 161, 186]),
                (0.52, [218, 210, 142]),
                (0.73, [134, 157, 89]),
                (1.00, [112, 86, 58]),
            ],
        ),
        PreviewMode::Elevation => gradient_color(
            signed_to_unit(site.base_fields.elevation_seed),
            &[
                (0.00, [35, 88, 127]),
                (0.32, [79, 141, 104]),
                (0.58, [181, 167, 100]),
                (0.80, [139, 124, 111]),
                (1.00, [241, 242, 232]),
            ],
        ),
        PreviewMode::Ruggedness => gradient_color(
            site.ruggedness.clamp(0.0, 1.0),
            &[
                (0.00, [87, 151, 116]),
                (0.42, [172, 178, 126]),
                (0.72, [139, 119, 104]),
                (1.00, [70, 70, 76]),
            ],
        ),
    }
}

fn color_for_identity_site(site: VoronoiSite) -> [u8; 3] {
    let identity = color_from_hash(site.id.0);
    let climate = [
        scale_channel(74, 0.55 + site.base_fields.temperature * 0.95),
        scale_channel(122, 0.55 + site.base_fields.hydration * 0.85),
        scale_channel(168, 0.48 + (1.0 - site.base_fields.continentality) * 0.70),
    ];
    let elevation =
        (0.82 + site.base_fields.elevation_seed * 0.20 + site.ruggedness * 0.10).clamp(0.62, 1.18);
    scale(blend(identity, climate, 0.62), elevation)
}

fn signed_to_unit(value: f32) -> f32 {
    ((value + 1.0) * 0.5).clamp(0.0, 1.0)
}

fn edge_overlay_amount(mode: PreviewMode) -> f32 {
    match mode {
        PreviewMode::Identity => 0.86,
        _ => 0.42,
    }
}

fn gradient_color(value: f32, stops: &[(f32, [u8; 3])]) -> [u8; 3] {
    debug_assert!(!stops.is_empty());
    let value = value.clamp(0.0, 1.0);

    for pair in stops.windows(2) {
        let (left_value, left_color) = pair[0];
        let (right_value, right_color) = pair[1];
        if value <= right_value {
            let span = (right_value - left_value).max(f32::EPSILON);
            let amount = ((value - left_value) / span).clamp(0.0, 1.0);
            return blend(left_color, right_color, amount);
        }
    }

    stops[stops.len() - 1].1
}

fn color_from_hash(hash: u64) -> [u8; 3] {
    let r = 72 + ((hash >> 8) & 0x7f) as u8;
    let g = 72 + ((hash >> 24) & 0x7f) as u8;
    let b = 72 + ((hash >> 40) & 0x7f) as u8;
    [r, g, b]
}

fn region_grid_strength(
    world_x: f32,
    world_z: f32,
    region_size_blocks: i32,
    pixel_span: f32,
) -> f32 {
    let size = region_size_blocks as f32;
    let threshold = pixel_span.max(1.0) * 1.15;
    let dx = distance_to_grid_line(world_x, size);
    let dz = distance_to_grid_line(world_z, size);
    let distance = dx.min(dz);
    if distance >= threshold {
        0.0
    } else {
        1.0 - distance / threshold
    }
}

fn distance_to_grid_line(value: f32, size: f32) -> f32 {
    let local = value.rem_euclid(size);
    local.min(size - local)
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
        "new-world graph_voronoi_preview".to_string(),
    )?;
    let mut png_writer = encoder.write_header()?;
    png_writer.write_image_data(image.as_raw())?;
    Ok(())
}

fn blend(base: [u8; 3], tint: [u8; 3], amount: f32) -> [u8; 3] {
    let amount = amount.clamp(0.0, 1.0);
    [
        mix_channel(base[0], tint[0], amount),
        mix_channel(base[1], tint[1], amount),
        mix_channel(base[2], tint[2], amount),
    ]
}

fn scale(color: [u8; 3], factor: f32) -> [u8; 3] {
    [
        scale_channel(color[0], factor),
        scale_channel(color[1], factor),
        scale_channel(color[2], factor),
    ]
}

fn mix_channel(base: u8, tint: u8, amount: f32) -> u8 {
    ((base as f32 * (1.0 - amount)) + (tint as f32 * amount))
        .round()
        .clamp(0.0, 255.0) as u8
}

fn scale_channel(channel: u8, factor: f32) -> u8 {
    (channel as f32 * factor).round().clamp(0.0, 255.0) as u8
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
    "usage: cargo run --bin graph_voronoi_preview -- <seed> <center-x> <center-z> [--width <u32>] [--height <u32>] [--world-span-blocks <i32>] [--region-size-blocks <i32>] [--site-spacing-blocks <i32>] [--stage graph_voronoi] [--mode <all|identity|temperature|hydration|humidity|continentality|elevation|ruggedness>] [--output <path>]"
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_window_is_4k() {
        let config = PreviewConfig {
            seed: 42,
            center_x: 0,
            center_z: 0,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            world_span_blocks: DEFAULT_WORLD_SPAN_BLOCKS,
            region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
            stage: DEFAULT_STAGE.to_string(),
            mode: PreviewModeSelection::Single(PreviewMode::Identity),
            output: None,
        }
        .validate()
        .expect("default config should be valid");

        assert_eq!(config.width, 3840);
        assert_eq!(config.height, 2160);
    }

    #[test]
    fn graph_preview_uses_stable_world_graph_patch() {
        let meta = WorldMeta::new(42);
        let config = PreviewConfig {
            seed: 42,
            center_x: 0,
            center_z: 0,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            world_span_blocks: DEFAULT_WORLD_SPAN_BLOCKS,
            region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
            stage: DEFAULT_STAGE.to_string(),
            mode: PreviewModeSelection::Single(PreviewMode::Identity),
            output: None,
        };
        let window = config.window();
        let area = window.graph_area(config.region_size_blocks).unwrap();

        let left = build_graph_patch_for_preview(&meta, &config, area).unwrap();
        let right = build_graph_patch_for_preview(&meta, &config, area).unwrap();

        assert_eq!(left.patch.sites, right.patch.sites);
        assert!(!left.patch.edges.is_empty());
    }

    #[test]
    fn default_output_path_preserves_seed_center_generator_stage() {
        let config = PreviewConfig {
            seed: 42,
            center_x: -10,
            center_z: 20,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            world_span_blocks: DEFAULT_WORLD_SPAN_BLOCKS,
            region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
            stage: DEFAULT_STAGE.to_string(),
            mode: PreviewModeSelection::Single(PreviewMode::Identity),
            output: None,
        };
        let path = config.output_path(11).display().to_string();

        assert!(path.contains("seed_42"));
        assert!(path.contains("cx-10"));
        assert!(path.contains("cz20"));
        assert!(path.contains("generator_gv11"));
        assert!(path.contains("stage_graph_voronoi"));
    }

    #[test]
    fn mode_all_expands_to_all_renderable_maps() {
        let selection = PreviewMode::parse("all").expect("all mode should parse");
        assert_eq!(selection.modes(), &RENDERABLE_MODES);
    }

    #[test]
    fn all_mode_writes_to_directory_paths() {
        let config = PreviewConfig {
            seed: 42,
            center_x: -10,
            center_z: 20,
            width: 64,
            height: 32,
            world_span_blocks: 512,
            region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
            stage: DEFAULT_STAGE.to_string(),
            mode: PreviewModeSelection::All,
            output: Some(PathBuf::from("target/graph-voronoi-preview/smoke")),
        };

        let paths = output_paths_for_config(&config, 11).unwrap();

        assert_eq!(paths.len(), RENDERABLE_MODES.len());
        assert!(
            paths
                .iter()
                .any(|(mode, path)| *mode == PreviewMode::Temperature
                    && path.display().to_string().contains("mode_temperature"))
        );
    }
}
