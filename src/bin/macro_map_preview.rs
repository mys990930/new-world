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
    GraphRegionCoord, MacroEdge, MacroMapConfig, MacroSite, MacroSurfaceKind, VoronoiGraphConfig,
    VoronoiGraphPatch, VoronoiGraphPatchRequest, VoronoiSiteId, WorldPlanePoint,
    generate_macro_map, generate_voronoi_graph_patch, graph_region_for_world_block,
};

const DEFAULT_WIDTH: u32 = 3840;
const DEFAULT_HEIGHT: u32 = 2160;
const DEFAULT_WORLD_SPAN_BLOCKS: i32 = 32768;
const DEFAULT_STAGE: &str = "macro_map";
const OUTPUT_DIR: &str = "target/macro-map-preview";
const SEA_LEVEL: f32 = 0.0;

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

    fn default_output_path(&self) -> PathBuf {
        PathBuf::from(format!(
            "{OUTPUT_DIR}/s{}_x{}_z{}.png",
            self.seed, self.center_x, self.center_z
        ))
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

    fn sample_world_x(self, pixel_x: u32) -> f32 {
        self.min_x() + (pixel_x as f32 + 0.5) * self.world_span_x / self.width as f32
    }

    fn sample_world_z(self, pixel_z: u32) -> f32 {
        self.min_z() + (pixel_z as f32 + 0.5) * self.world_span_z / self.height as f32
    }

    fn world_segment_to_pixels(
        self,
        a: WorldPlanePoint,
        b: WorldPlanePoint,
    ) -> Option<((i32, i32), (i32, i32))> {
        let (a, b) = self.clip_world_segment(a, b)?;
        Some((
            self.world_to_pixel_clamped(a),
            self.world_to_pixel_clamped(b),
        ))
    }

    fn clip_world_segment(
        self,
        a: WorldPlanePoint,
        b: WorldPlanePoint,
    ) -> Option<(WorldPlanePoint, WorldPlanePoint)> {
        let dx = b.x - a.x;
        let dz = b.z - a.z;
        let mut enter = 0.0;
        let mut exit = 1.0;

        if !clip_segment_axis(-dx, a.x - self.min_x(), &mut enter, &mut exit)
            || !clip_segment_axis(dx, self.max_x() - a.x, &mut enter, &mut exit)
            || !clip_segment_axis(-dz, a.z - self.min_z(), &mut enter, &mut exit)
            || !clip_segment_axis(dz, self.max_z() - a.z, &mut enter, &mut exit)
        {
            return None;
        }

        Some((
            WorldPlanePoint::new(a.x + dx * enter, a.z + dz * enter),
            WorldPlanePoint::new(a.x + dx * exit, a.z + dz * exit),
        ))
    }

    fn world_to_pixel_clamped(self, point: WorldPlanePoint) -> (i32, i32) {
        let x = ((point.x - self.min_x()) / self.world_span_x * self.width as f32 - 0.5)
            .round()
            .clamp(0.0, self.width.saturating_sub(1) as f32) as i32;
        let y = ((point.z - self.min_z()) / self.world_span_z * self.height as f32 - 0.5)
            .round()
            .clamp(0.0, self.height.saturating_sub(1) as f32) as i32;
        (x, y)
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
        GraphRegionArea::new(min, max).ok_or_else(|| cli_error("invalid macro map preview area"))
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SiteGridCoord {
    x: i32,
    z: i32,
}

#[derive(Debug, Clone)]
struct PreviewGraph {
    patch: VoronoiGraphPatch,
    site_grid: HashMap<SiteGridCoord, usize>,
    site_samples: HashMap<VoronoiSiteId, MacroSite>,
    edge_samples: Vec<MacroEdgeSample>,
    spacing: f32,
}

#[derive(Debug, Clone, Copy)]
struct NearestSite {
    index: usize,
    distance_sq: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EdgeKind {
    Coast,
    Mountain,
    Ridge,
    Fault,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MacroEdgeSample {
    edge: MacroEdge,
    kind: EdgeKind,
    strength: f32,
}

#[derive(Debug, Clone)]
struct PreviewHeader {
    seed: u64,
    generator_version: u32,
    stage: String,
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
    edge_count: usize,
    coast_edge_count: usize,
    mountain_edge_count: usize,
    ridge_edge_count: usize,
    fault_edge_count: usize,
    macro_source: &'static str,
}

impl PreviewHeader {
    fn to_metadata_text(&self) -> String {
        [
            "binary=macro_map_preview".to_string(),
            format!("seed={}", self.seed),
            format!("generator_version={}", self.generator_version),
            format!("stage={}", self.stage),
            "map_name=macro map composite".to_string(),
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
            format!("candidate_edge_count={}", self.edge_count),
            format!("coast_edge_count={}", self.coast_edge_count),
            format!("mountain_edge_count={}", self.mountain_edge_count),
            format!("ridge_edge_count={}", self.ridge_edge_count),
            format!("fault_edge_count={}", self.fault_edge_count),
            format!("sea_level={SEA_LEVEL}"),
            "stage4_guide_inputs=component,inlandness,signed_elevation_gradient,mountainness,ridgeness,basinness,drainage_divide_potential".to_string(),
            format!("macro_source={}", self.macro_source),
            "world_api=new_world::world::generation::generate_macro_map(patch, config)".to_string(),
        ]
        .join("\n")
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?.validate()?;
    let meta = WorldMeta::new(config.seed);
    let window = config.window();
    let graph_area = window.graph_area(config.region_size_blocks)?;
    let graph = build_macro_map_for_preview(&meta, &config, graph_area)?;
    let output = output_path_for_config(&config);
    let coast_edge_count = graph
        .edge_samples
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Coast)
        .count();
    let mountain_edge_count = graph
        .edge_samples
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Mountain)
        .count();
    let ridge_edge_count = graph
        .edge_samples
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Ridge)
        .count();
    let fault_edge_count = graph
        .edge_samples
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Fault)
        .count();

    let header = PreviewHeader {
        seed: config.seed,
        generator_version: meta.generator_version,
        stage: config.stage.clone(),
        center_x: config.center_x,
        center_z: config.center_z,
        width: config.width,
        height: config.height,
        world_span_blocks: config.world_span_blocks,
        region_size_blocks: config.region_size_blocks,
        site_spacing_blocks: config.site_spacing_blocks,
        land_bias: config.land_bias,
        graph_area,
        site_count: graph.patch.sites.len(),
        edge_count: graph.edge_samples.len(),
        coast_edge_count,
        mountain_edge_count,
        ridge_edge_count,
        fault_edge_count,
        macro_source: "world_generation_macro_map",
    };

    let mut image = render_preview(window, &graph)?;
    draw_candidate_edges(&mut image, window, &graph);
    draw_legend_overlay(&mut image);
    write_png_with_metadata(&image, &output, &header)?;

    println!("seed: {}", config.seed);
    println!("generator version: {}", meta.generator_version);
    println!("stage: {}", config.stage);
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
        "sites: {}, candidate edges: {} (coast {}, mountain {}, ridge {}, fault {})",
        graph.patch.sites.len(),
        graph.edge_samples.len(),
        coast_edge_count,
        mountain_edge_count,
        ridge_edge_count,
        fault_edge_count
    );
    println!("metadata: new-world-preview-header iTXt chunk");
    println!(
        "generated file: {}x{} {}",
        image.width(),
        image.height(),
        output.display()
    );

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
    let mut land_bias = MacroMapConfig::new(seed, 0).land_bias;
    let mut stage = DEFAULT_STAGE.to_string();
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
        output,
    })
}

fn output_path_for_config(config: &PreviewConfig) -> PathBuf {
    config.output.as_ref().map_or_else(
        || config.default_output_path(),
        |path| {
            if looks_like_file(path) {
                path.clone()
            } else {
                path.join(format!(
                    "s{}_x{}_z{}.png",
                    config.seed, config.center_x, config.center_z
                ))
            }
        },
    )
}

fn looks_like_file(path: &Path) -> bool {
    path.extension().is_some()
}

fn build_macro_map_for_preview(
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

    let macro_map = generate_macro_map(&patch, macro_map_config_for_preview(meta, config));
    let site_samples = macro_map
        .sites
        .iter()
        .map(|site| (site.id, *site))
        .collect::<HashMap<_, _>>();
    let mut edge_samples = macro_map
        .edges
        .par_iter()
        .filter_map(|edge| macro_edge_sample(*edge))
        .collect::<Vec<_>>();
    edge_samples.sort_by_key(|sample| (edge_kind_draw_order(sample.kind), sample.edge.id.0));

    Ok(PreviewGraph {
        patch,
        site_grid,
        site_samples,
        edge_samples,
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
        .map_err(|_| cli_error("macro map preview padding overflowed"))
}

fn site_grid_coord_for_position(position: WorldPlanePoint, spacing: f32) -> SiteGridCoord {
    SiteGridCoord {
        x: (position.x / spacing).floor() as i32,
        z: (position.z / spacing).floor() as i32,
    }
}

fn macro_edge_sample(edge: MacroEdge) -> Option<MacroEdgeSample> {
    if edge.guide.is_coast {
        return Some(MacroEdgeSample {
            edge,
            kind: EdgeKind::Coast,
            strength: edge.guide.coastness,
        });
    }
    if edge.guide.is_fault_candidate {
        return Some(MacroEdgeSample {
            edge,
            kind: EdgeKind::Fault,
            strength: edge.guide.signed_elevation_gradient.abs(),
        });
    }
    if edge.guide.is_ridge_candidate {
        return Some(MacroEdgeSample {
            edge,
            kind: EdgeKind::Ridge,
            strength: edge.guide.ridgeness,
        });
    }
    if edge.guide.is_mountain_candidate {
        return Some(MacroEdgeSample {
            edge,
            kind: EdgeKind::Mountain,
            strength: edge.guide.mountainness,
        });
    }
    None
}

fn macro_map_config_for_preview(meta: &WorldMeta, config: &PreviewConfig) -> MacroMapConfig {
    MacroMapConfig {
        land_bias: config.land_bias,
        ..MacroMapConfig::new(meta.seed, meta.generator_version)
    }
}

fn edge_kind_draw_order(kind: EdgeKind) -> u8 {
    match kind {
        EdgeKind::Coast => 0,
        EdgeKind::Mountain => 1,
        EdgeKind::Ridge => 2,
        EdgeKind::Fault => 3,
    }
}

fn render_preview(window: PreviewWindow, graph: &PreviewGraph) -> Result<RgbImage, Box<dyn Error>> {
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
            let site = nearest_site(graph, world_x, world_z);
            let macro_site = graph
                .site_samples
                .get(&graph.patch.sites[site.index].id)
                .copied()
                .expect("macro map should contain every graph site");
            let mut color = color_for_macro_site(macro_site);
            let center_marker = site.distance_sq.sqrt() / graph.spacing;
            if center_marker < 0.028 {
                color = blend(color, [248, 248, 232], 0.65);
            }
            pixel.copy_from_slice(&color);
        });

    RgbImage::from_raw(window.width, window.height, pixels)
        .ok_or_else(|| cli_error("failed to build RGB image"))
}

fn nearest_site(graph: &PreviewGraph, world_x: f32, world_z: f32) -> NearestSite {
    let center_cell_x = (world_x / graph.spacing).floor() as i32;
    let center_cell_z = (world_z / graph.spacing).floor() as i32;
    let mut nearest_index = 0;
    let mut nearest_distance_sq = f32::MAX;

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
                nearest_distance_sq = distance_sq;
                nearest_index = index;
            }
        }
    }

    NearestSite {
        index: nearest_index,
        distance_sq: nearest_distance_sq,
    }
}

fn color_for_macro_site(site: MacroSite) -> [u8; 3] {
    let elevation = signed_to_unit(site.signed_macro_elevation);
    match site.surface_kind {
        MacroSurfaceKind::OceanBasin => gradient_color(
            elevation,
            &[
                (0.00, [18, 54, 112]),
                (0.52, [32, 104, 174]),
                (1.00, [80, 169, 205]),
            ],
        ),
        MacroSurfaceKind::LakeCandidate => {
            gradient_color(elevation, &[(0.00, [42, 119, 183]), (1.00, [94, 188, 218])])
        }
        MacroSurfaceKind::CoastOcean => gradient_color(
            elevation,
            &[
                (0.00, [44, 128, 181]),
                (0.62, [220, 205, 139]),
                (1.00, [226, 212, 150]),
            ],
        ),
        MacroSurfaceKind::CoastLand => gradient_color(
            elevation,
            &[
                (0.00, [104, 166, 178]),
                (0.48, [220, 205, 139]),
                (1.00, [117, 166, 99]),
            ],
        ),
        MacroSurfaceKind::CoastIsland => gradient_color(
            elevation,
            &[
                (0.00, [88, 158, 174]),
                (0.50, [224, 210, 146]),
                (1.00, [125, 174, 104]),
            ],
        ),
        MacroSurfaceKind::WetlandCandidate => gradient_color(
            elevation,
            &[
                (0.00, [64, 145, 135]),
                (0.55, [93, 156, 104]),
                (1.00, [160, 181, 121]),
            ],
        ),
        MacroSurfaceKind::Island => gradient_color(
            elevation,
            &[
                (0.00, [94, 153, 92]),
                (0.56, [138, 176, 95]),
                (0.82, [177, 175, 135]),
                (1.00, [236, 238, 221]),
            ],
        ),
        MacroSurfaceKind::Continent => gradient_color(
            elevation,
            &[
                (0.00, [82, 142, 76]),
                (0.42, [112, 166, 82]),
                (0.66, [156, 154, 109]),
                (0.82, [164, 164, 158]),
                (1.00, [248, 249, 242]),
            ],
        ),
    }
}

fn signed_to_unit(value: f32) -> f32 {
    ((value + 1.0) * 0.5).clamp(0.0, 1.0)
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

fn draw_candidate_edges(image: &mut RgbImage, window: PreviewWindow, graph: &PreviewGraph) {
    let corners = graph
        .patch
        .corners
        .iter()
        .map(|corner| (corner.id, corner.position))
        .collect::<HashMap<_, _>>();

    for sample in &graph.edge_samples {
        let Some(a) = corners.get(&sample.edge.corners[0]).copied() else {
            continue;
        };
        let Some(b) = corners.get(&sample.edge.corners[1]).copied() else {
            continue;
        };
        let Some((start, end)) = window.world_segment_to_pixels(a, b) else {
            continue;
        };
        let color = match sample.kind {
            EdgeKind::Coast => [236, 213, 128],
            EdgeKind::Mountain => [207, 176, 93],
            EdgeKind::Ridge => [247, 248, 242],
            EdgeKind::Fault => [231, 92, 88],
        };
        let width = match sample.kind {
            EdgeKind::Coast => 1,
            EdgeKind::Mountain => 1,
            EdgeKind::Ridge => 2,
            EdgeKind::Fault => 2,
        };
        draw_line(
            image,
            start,
            end,
            color,
            (0.48 + sample.strength * 0.34).clamp(0.0, 1.0),
            width,
        );
    }
}

fn draw_line(
    image: &mut RgbImage,
    start: (i32, i32),
    end: (i32, i32),
    color: [u8; 3],
    amount: f32,
    width: i32,
) {
    let (mut x0, mut y0) = start;
    let (x1, y1) = end;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        for oy in -width..=width {
            for ox in -width..=width {
                if ox * ox + oy * oy <= width * width {
                    blend_pixel_i32(image, x0 + ox, y0 + oy, color, amount);
                }
            }
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn draw_legend_overlay(image: &mut RgbImage) {
    if image.width() < 120 || image.height() < 72 {
        return;
    }

    let scale = if image.width() >= 640 && image.height() >= 360 {
        2
    } else {
        1
    };
    let margin = 8 * scale;
    let panel_width = (160 * scale).min(image.width());
    let panel_height = (78 * scale).min(image.height());
    let x = margin.min(image.width().saturating_sub(panel_width));
    let y = margin.min(image.height().saturating_sub(panel_height));

    blend_rect(image, x, y, panel_width, panel_height, [10, 13, 18], 0.72);
    draw_text(
        image,
        x + 7 * scale,
        y + 6 * scale,
        "MACRO MAP",
        [238, 241, 232],
        scale,
    );

    let bar_x = x + 8 * scale;
    let bar_y = y + 20 * scale;
    let bar_width = panel_width.saturating_sub(16 * scale).max(1);
    let bar_height = 7 * scale;
    draw_elevation_bar(image, bar_x, bar_y, bar_width, bar_height);
    draw_text(
        image,
        bar_x,
        bar_y + bar_height + 5 * scale,
        "OCEAN",
        [218, 224, 212],
        scale,
    );
    let right = "PEAK";
    let right_width = text_width(right, scale);
    draw_text(
        image,
        bar_x + bar_width.saturating_sub(right_width),
        bar_y + bar_height + 5 * scale,
        right,
        [218, 224, 212],
        scale,
    );

    let key_y = y + panel_height.saturating_sub(30 * scale);
    draw_key(image, bar_x, key_y, [247, 248, 242], "RIDGE", scale);
    draw_key(
        image,
        bar_x + 68 * scale,
        key_y,
        [231, 92, 88],
        "FAULT",
        scale,
    );
    draw_key(
        image,
        bar_x,
        key_y + 13 * scale,
        [207, 176, 93],
        "MTN",
        scale,
    );
    draw_key(
        image,
        bar_x + 68 * scale,
        key_y + 13 * scale,
        [236, 213, 128],
        "COAST",
        scale,
    );
}

fn draw_key(image: &mut RgbImage, x: u32, y: u32, color: [u8; 3], label: &str, scale: u32) {
    blend_rect(image, x, y + 2 * scale, 8 * scale, 3 * scale, color, 0.95);
    draw_text(image, x + 11 * scale, y, label, [218, 224, 212], scale);
}

fn draw_elevation_bar(image: &mut RgbImage, x: u32, y: u32, width: u32, height: u32) {
    let max_x = (x + width).min(image.width());
    let max_y = (y + height).min(image.height());
    let denom = width.saturating_sub(1).max(1) as f32;
    for py in y..max_y {
        for px in x..max_x {
            let t = (px - x) as f32 / denom;
            set_pixel(
                image,
                px,
                py,
                gradient_color(
                    t,
                    &[
                        (0.00, [18, 54, 112]),
                        (0.35, [46, 137, 188]),
                        (0.45, [220, 205, 139]),
                        (0.68, [112, 166, 82]),
                        (0.84, [164, 164, 158]),
                        (1.00, [248, 249, 242]),
                    ],
                ),
            );
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
                    let px = x + col * scale + sx;
                    let py = y + row as u32 * scale + sy;
                    blend_pixel(image, px, py, color, 0.95);
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
        "new-world macro_map_preview".to_string(),
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

fn mix_channel(base: u8, tint: u8, amount: f32) -> u8 {
    ((base as f32 * (1.0 - amount)) + (tint as f32 * amount))
        .round()
        .clamp(0.0, 255.0) as u8
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
    "usage: cargo run --bin macro_map_preview -- <seed> <center-x> <center-z> [--width <u32>] [--height <u32>] [--world-span-blocks <i32>] [--region-size-blocks <i32>] [--site-spacing-blocks <i32>] [--land-bias <f32>] [--stage macro_map] [--output <path>]"
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
            width: 128,
            height: 72,
            world_span_blocks: 512,
            region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
            land_bias: MacroMapConfig::new(42, 0).land_bias,
            stage: DEFAULT_STAGE.to_string(),
            output: None,
        }
    }

    #[test]
    fn default_output_path_uses_short_seed_center_name() {
        let path = test_config().default_output_path().display().to_string();

        assert!(path.ends_with("target/macro-map-preview/s42_x-10_z20.png"));
        assert!(!path.contains("generator_gv"));
        assert!(!path.contains("span"));
    }

    #[test]
    fn explicit_directory_output_uses_default_short_file_name() {
        let mut config = test_config();
        config.output = Some(PathBuf::from("target/macro-map-preview/smoke"));

        let path = output_path_for_config(&config)
            .display()
            .to_string()
            .replace('\\', "/");

        assert!(path.ends_with("target/macro-map-preview/smoke/s42_x-10_z20.png"));
    }

    #[test]
    fn explicit_png_output_path_is_preserved() {
        let mut config = test_config();
        config.output = Some(PathBuf::from("target/custom/macro.png"));

        assert_eq!(
            output_path_for_config(&config),
            PathBuf::from("target/custom/macro.png")
        );
    }

    #[test]
    fn preview_macro_map_config_passes_land_tuning() {
        let meta = WorldMeta::new(42);
        let mut config = test_config();
        config.land_bias = -0.18;

        let macro_config = macro_map_config_for_preview(&meta, &config);

        assert_eq!(macro_config.land_bias, -0.18);
        assert_eq!(macro_config.seed, meta.seed);
        assert_eq!(macro_config.generator_version, meta.generator_version);
    }

    #[test]
    fn elevation_gradient_reaches_ocean_and_peak_colors() {
        assert_eq!(
            gradient_color(0.0, &[(0.0, [18, 54, 112]), (1.0, [248, 249, 242])]),
            [18, 54, 112]
        );
        assert_eq!(
            gradient_color(1.0, &[(0.0, [18, 54, 112]), (1.0, [248, 249, 242])]),
            [248, 249, 242]
        );
    }

    #[test]
    fn legend_overlay_changes_image_pixels() {
        let mut image = RgbImage::from_pixel(180, 90, image::Rgb([4, 5, 6]));

        draw_legend_overlay(&mut image);

        assert_ne!(image.as_raw(), &vec![4_u8, 5, 6].repeat(180 * 90));
    }

    #[test]
    fn world_segment_projection_matches_pixel_sample_centers() {
        let window = PreviewWindow {
            center_x: 0.0,
            center_z: 0.0,
            width: 100,
            height: 50,
            world_span_x: 200.0,
            world_span_z: 100.0,
        };
        let pixel_x = 12;
        let pixel_y = 7;
        let point = WorldPlanePoint::new(
            window.sample_world_x(pixel_x),
            window.sample_world_z(pixel_y),
        );

        assert_eq!(window.world_to_pixel_clamped(point), (12, 7));
    }

    #[test]
    fn world_segment_projection_clips_edges_crossing_the_preview_window() {
        let window = PreviewWindow {
            center_x: 0.0,
            center_z: 0.0,
            width: 10,
            height: 10,
            world_span_x: 100.0,
            world_span_z: 100.0,
        };

        let segment = window
            .world_segment_to_pixels(
                WorldPlanePoint::new(-100.0, 0.0),
                WorldPlanePoint::new(100.0, 0.0),
            )
            .expect("crossing graph edge should be clipped instead of dropped");

        assert_eq!(segment, ((0, 5), (9, 5)));
    }

    #[test]
    fn world_segment_projection_drops_edges_outside_the_preview_window() {
        let window = PreviewWindow {
            center_x: 0.0,
            center_z: 0.0,
            width: 10,
            height: 10,
            world_span_x: 100.0,
            world_span_z: 100.0,
        };

        assert_eq!(
            window.world_segment_to_pixels(
                WorldPlanePoint::new(-100.0, -100.0),
                WorldPlanePoint::new(-75.0, -75.0),
            ),
            None
        );
    }

    #[test]
    fn graph_derived_macro_map_is_deterministic() {
        let meta = WorldMeta::new(42);
        let config = test_config();
        let area = config
            .window()
            .graph_area(config.region_size_blocks)
            .unwrap();

        let left = build_macro_map_for_preview(&meta, &config, area).unwrap();
        let right = build_macro_map_for_preview(&meta, &config, area).unwrap();

        assert_eq!(left.patch.sites, right.patch.sites);
        assert_eq!(left.site_samples, right.site_samples);
        assert_eq!(left.edge_samples, right.edge_samples);
    }
}
