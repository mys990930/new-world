use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, ErrorKind};
use std::path::{Path, PathBuf};

use image::RgbImage;
use rayon::prelude::*;

use new_world::world::WorldMeta;
use new_world::world::generation::{
    DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS, GraphBiomeCell, GraphBiomeKind,
    GraphRegionArea, GraphRegionCoord, HydrologyConfig, MacroMapConfig, MacroSite,
    VoronoiGraphConfig, VoronoiGraphPatch, VoronoiGraphPatchRequest, VoronoiSiteId,
    WorldPlanePoint, apply_headwater_source_hydration_to_biomes, generate_macro_map,
    generate_voronoi_graph_patch, graph_region_for_world_block, solve_hydrology,
};

mod common;

use common::preview_compass::draw_compass_rgb;
use common::preview_draw::{blend_pixel_i32, blend_rect, draw_text, set_pixel};

const DEFAULT_WIDTH: u32 = 3840;
const DEFAULT_HEIGHT: u32 = 2160;
const DEFAULT_WORLD_SPAN_BLOCKS: i32 = 32768;
const DEFAULT_STAGE: &str = "biome_map";
const OUTPUT_DIR: &str = "target/biome-map-preview";
const BIOME_PALETTE: [BiomePaletteEntry; 30] = [
    BiomePaletteEntry::new(GraphBiomeKind::DeepOcean, "DEEP-OCEAN", [0, 32, 96]),
    BiomePaletteEntry::new(GraphBiomeKind::ShallowOcean, "SHALLOW-OCN", [0, 147, 196]),
    BiomePaletteEntry::new(GraphBiomeKind::Mangrove, "MANGROVE", [0, 86, 63]),
    BiomePaletteEntry::new(GraphBiomeKind::EstuarineCoast, "ESTUARY", [96, 171, 130]),
    BiomePaletteEntry::new(GraphBiomeKind::LagoonCoast, "LAGOON", [85, 210, 198]),
    BiomePaletteEntry::new(GraphBiomeKind::RockyCoast, "ROCKY-COAST", [115, 118, 130]),
    BiomePaletteEntry::new(GraphBiomeKind::SandyCoast, "SANDY-COAST", [238, 213, 132]),
    BiomePaletteEntry::new(GraphBiomeKind::Lake, "LAKE", [52, 88, 209]),
    BiomePaletteEntry::new(GraphBiomeKind::Marsh, "MARSH", [116, 150, 110]),
    BiomePaletteEntry::new(GraphBiomeKind::Swamp, "SWAMP", [57, 69, 42]),
    BiomePaletteEntry::new(GraphBiomeKind::FloodedForest, "FLOOD-FRST", [32, 78, 136]),
    BiomePaletteEntry::new(GraphBiomeKind::Desert, "DESERT", [224, 173, 43]),
    BiomePaletteEntry::new(GraphBiomeKind::SemiDesert, "SEMI-DESERT", [190, 119, 57]),
    BiomePaletteEntry::new(GraphBiomeKind::Steppe, "STEPPE", [154, 178, 105]),
    BiomePaletteEntry::new(GraphBiomeKind::DryShrubland, "DRY-SHRUB", [136, 88, 52]),
    BiomePaletteEntry::new(
        GraphBiomeKind::MediterraneanShrubland,
        "MED-SHRUB",
        [104, 116, 38],
    ),
    BiomePaletteEntry::new(GraphBiomeKind::PolarIce, "POLAR-ICE", [232, 245, 250]),
    BiomePaletteEntry::new(GraphBiomeKind::PolarBarrens, "POLAR-BARR", [188, 188, 188]),
    BiomePaletteEntry::new(GraphBiomeKind::Tundra, "TUNDRA", [169, 178, 153]),
    BiomePaletteEntry::new(
        GraphBiomeKind::SubalpineWoodland,
        "SUBALPINE",
        [52, 106, 94],
    ),
    BiomePaletteEntry::new(GraphBiomeKind::AlpineMeadow, "ALP-MEADOW", [144, 128, 166]),
    BiomePaletteEntry::new(GraphBiomeKind::BorealForest, "BOREAL-FRST", [24, 80, 98]),
    BiomePaletteEntry::new(
        GraphBiomeKind::TropicalRainforest,
        "TROP-RAIN",
        [0, 116, 54],
    ),
    BiomePaletteEntry::new(GraphBiomeKind::MonsoonForest, "MONSOON", [38, 156, 69]),
    BiomePaletteEntry::new(
        GraphBiomeKind::TropicalDryForest,
        "TROP-DRY",
        [108, 162, 39],
    ),
    BiomePaletteEntry::new(GraphBiomeKind::Savanna, "SAVANNA", [201, 190, 55]),
    BiomePaletteEntry::new(
        GraphBiomeKind::TemperateRainforest,
        "TEMP-RAIN",
        [0, 128, 116],
    ),
    BiomePaletteEntry::new(
        GraphBiomeKind::TemperateMixedForest,
        "TEMP-MIXED",
        [48, 132, 47],
    ),
    BiomePaletteEntry::new(
        GraphBiomeKind::TemperateBroadleafForest,
        "TEMP-BROAD",
        [81, 154, 64],
    ),
    BiomePaletteEntry::new(
        GraphBiomeKind::TemperateGrassland,
        "TEMP-GRASS",
        [130, 190, 89],
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BiomePaletteEntry {
    biome: GraphBiomeKind,
    label: &'static str,
    color: [u8; 3],
}

impl BiomePaletteEntry {
    const fn new(biome: GraphBiomeKind, label: &'static str, color: [u8; 3]) -> Self {
        Self {
            biome,
            label,
            color,
        }
    }
}

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
        self.max_z() - (pixel_z as f32 + 0.5) * self.world_span_z / self.height as f32
    }

    fn world_to_pixel_clamped(self, point: WorldPlanePoint) -> (i32, i32) {
        let x = ((point.x - self.min_x()) / self.world_span_x * self.width as f32 - 0.5)
            .round()
            .clamp(0.0, self.width.saturating_sub(1) as f32) as i32;
        let y = ((self.max_z() - point.z) / self.world_span_z * self.height as f32 - 0.5)
            .round()
            .clamp(0.0, self.height.saturating_sub(1) as f32) as i32;
        (x, y)
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
        GraphRegionArea::new(min, max).ok_or_else(|| cli_error("invalid biome map preview area"))
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
    biome_sites: HashMap<VoronoiSiteId, GraphBiomeCell>,
    macro_sites: HashMap<VoronoiSiteId, MacroSite>,
    spacing: f32,
}

#[derive(Debug, Clone, Copy)]
struct NearestSite {
    index: usize,
    distance_sq: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PreviewBiomeStats {
    ocean: usize,
    lake: usize,
    wetland: usize,
    dry_basin: usize,
    beach: usize,
    desert: usize,
    savanna: usize,
    grassland: usize,
    temperate_forest: usize,
    boreal_forest: usize,
    rainforest: usize,
    tundra: usize,
    alpine: usize,
    mountain: usize,
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
    stats: PreviewBiomeStats,
    biome_source: &'static str,
}

impl PreviewHeader {
    fn to_metadata_text(&self) -> String {
        [
            "binary=biome_map_preview".to_string(),
            format!("seed={}", self.seed),
            format!("generator_version={}", self.generator_version),
            format!("stage={}", self.stage),
            "map_name=biome map by resolved Voronoi cell".to_string(),
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
            format!("biome_ocean_count={}", self.stats.ocean),
            format!("biome_lake_count={}", self.stats.lake),
            format!("biome_wetland_count={}", self.stats.wetland),
            format!("biome_dry_basin_count={}", self.stats.dry_basin),
            format!("biome_beach_count={}", self.stats.beach),
            format!("biome_desert_count={}", self.stats.desert),
            format!("biome_savanna_count={}", self.stats.savanna),
            format!("biome_grassland_count={}", self.stats.grassland),
            format!(
                "biome_temperate_forest_count={}",
                self.stats.temperate_forest
            ),
            format!("biome_boreal_forest_count={}", self.stats.boreal_forest),
            format!("biome_rainforest_count={}", self.stats.rainforest),
            format!("biome_tundra_count={}", self.stats.tundra),
            format!("biome_alpine_count={}", self.stats.alpine),
            format!("biome_mountain_count={}", self.stats.mountain),
            format!("biome_source={}", self.biome_source),
            "world_api=new_world::world::generation::{GraphBiomeCell,GraphBiomeKind,classify_graph_biome}"
                .to_string(),
            "biome_map_source=GraphMacroMap.biomes".to_string(),
        ]
        .join("\n")
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?.validate()?;
    let meta = WorldMeta::new(config.seed);
    let window = config.window();
    let graph_area = window.graph_area(config.region_size_blocks)?;
    let graph = build_biome_map_for_preview(&meta, &config, graph_area)?;
    let output = output_path_for_config(&config);
    let stats = preview_biome_stats(graph.biome_sites.values());
    let header = PreviewHeader {
        seed: meta.seed,
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
        stats,
        biome_source: "world_generation_macro_map_biomes",
    };

    let mut image = render_preview(window, &graph)?;
    draw_base_voronoi_edges(&mut image, window, &graph);
    draw_legend_overlay(&mut image);
    draw_compass_rgb(&mut image);
    write_png_with_metadata(&image, &output, &header)?;
    print_summary(&output, &header);

    Ok(())
}

fn build_biome_map_for_preview(
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

    let mut macro_map = generate_macro_map(&patch, macro_map_config_for_preview(meta, config));
    let hydrology = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());
    apply_headwater_source_hydration_to_biomes(&patch, &mut macro_map, &hydrology);
    if patch
        .sites
        .iter()
        .any(|site| macro_map.biome(site.id).is_none())
    {
        return Err(cli_error(
            "generated biome map did not contain every graph site",
        ));
    }
    let biome_sites = macro_map
        .biomes
        .iter()
        .map(|site| (site.site, *site))
        .collect::<HashMap<_, _>>();
    let macro_sites = macro_map
        .sites
        .iter()
        .map(|site| (site.id, *site))
        .collect::<HashMap<_, _>>();

    Ok(PreviewGraph {
        patch,
        site_grid,
        biome_sites,
        macro_sites,
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
        .map_err(|_| cli_error("biome map preview padding overflowed"))
}

fn site_grid_coord_for_position(position: WorldPlanePoint, spacing: f32) -> SiteGridCoord {
    SiteGridCoord {
        x: (position.x / spacing).floor() as i32,
        z: (position.z / spacing).floor() as i32,
    }
}

fn macro_map_config_for_preview(meta: &WorldMeta, config: &PreviewConfig) -> MacroMapConfig {
    MacroMapConfig {
        land_bias: config.land_bias,
        ..MacroMapConfig::new(meta.seed, meta.generator_version)
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
            let site_id = graph.patch.sites[site.index].id;
            let biome_site = graph
                .biome_sites
                .get(&site_id)
                .copied()
                .expect("biome map should contain every graph site");
            let macro_site = graph.macro_sites.get(&site_id).copied();
            let mut color = color_for_biome_site(biome_site, macro_site);
            let center_marker = site.distance_sq.sqrt() / graph.spacing;
            if center_marker < 0.028 {
                color = blend(color, [248, 248, 232], 0.62);
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

fn color_for_biome_site(site: GraphBiomeCell, macro_site: Option<MacroSite>) -> [u8; 3] {
    let elevation = macro_site
        .map(|site| signed_to_unit(site.signed_macro_elevation))
        .unwrap_or_else(|| signed_to_unit(site.context.elevation));
    let base = color_for_biome_kind(site.biome);

    if matches!(
        site.biome,
        GraphBiomeKind::DeepOcean
            | GraphBiomeKind::ShallowOcean
            | GraphBiomeKind::Lake
            | GraphBiomeKind::Marsh
            | GraphBiomeKind::Swamp
            | GraphBiomeKind::FloodedForest
    ) {
        return base;
    }

    blend(base, [248, 249, 242], (elevation - 0.72).max(0.0) * 1.25)
}

fn color_for_biome_kind(biome: GraphBiomeKind) -> [u8; 3] {
    BIOME_PALETTE
        .iter()
        .find(|entry| entry.biome == biome)
        .map(|entry| entry.color)
        .expect("palette should contain every GraphBiomeKind")
}

fn draw_base_voronoi_edges(image: &mut RgbImage, window: PreviewWindow, graph: &PreviewGraph) {
    let corners = graph
        .patch
        .corners
        .iter()
        .map(|corner| (corner.id, corner.position))
        .collect::<HashMap<_, _>>();

    for edge in &graph.patch.edges {
        let Some(a) = corners.get(&edge.corners[0]).copied() else {
            continue;
        };
        let Some(b) = corners.get(&edge.corners[1]).copied() else {
            continue;
        };
        let Some((start, end)) = window.world_segment_to_pixels(a, b) else {
            continue;
        };
        draw_line(image, start, end, [19, 27, 32], 0.28, 1);
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
    let mut x = start.0;
    let mut y = start.1;
    let dx = (end.0 - start.0).abs();
    let sx = if start.0 < end.0 { 1 } else { -1 };
    let dy = -(end.1 - start.1).abs();
    let sy = if start.1 < end.1 { 1 } else { -1 };
    let mut err = dx + dy;
    let radius = width.max(1) / 2;

    loop {
        for oy in -radius..=radius {
            for ox in -radius..=radius {
                blend_pixel_i32(image, x + ox, y + oy, color, amount);
            }
        }
        if x == end.0 && y == end.1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

fn draw_legend_overlay(image: &mut RgbImage) {
    let x = 24;
    let y = 24;
    let title_scale = 3;
    let label_scale = 2;
    let swatch_width = 26;
    let swatch_height = 16;
    let row_step = 24;
    let columns = 2_u32;
    let rows = BIOME_PALETTE.len().div_ceil(columns as usize) as u32;
    let column_width = 246;
    let panel_width = columns * column_width + 16;
    let panel_height = rows * row_step + 50;
    blend_rect(
        image,
        x - 12,
        y - 12,
        panel_width,
        panel_height,
        [7, 10, 12],
        0.68,
    );
    draw_text(image, x, y, "BIOME MAP", [246, 246, 232], title_scale);
    for (index, entry) in BIOME_PALETTE.iter().enumerate() {
        let column = index as u32 / rows;
        let row = index as u32 % rows;
        let entry_x = x + column * column_width;
        let row_y = y + 38 + row * row_step;
        fill_rect(
            image,
            entry_x,
            row_y,
            swatch_width,
            swatch_height,
            entry.color,
        );
        draw_text(
            image,
            entry_x + swatch_width + 12,
            row_y,
            entry.label,
            [238, 238, 222],
            label_scale,
        );
    }
}

fn preview_biome_stats<'a>(sites: impl Iterator<Item = &'a GraphBiomeCell>) -> PreviewBiomeStats {
    let mut stats = PreviewBiomeStats::default();
    for site in sites {
        match site.biome {
            GraphBiomeKind::DeepOcean | GraphBiomeKind::ShallowOcean => stats.ocean += 1,
            GraphBiomeKind::Lake => stats.lake += 1,
            GraphBiomeKind::Marsh | GraphBiomeKind::Swamp | GraphBiomeKind::FloodedForest => {
                stats.wetland += 1
            }
            GraphBiomeKind::SemiDesert
            | GraphBiomeKind::Steppe
            | GraphBiomeKind::DryShrubland
            | GraphBiomeKind::MediterraneanShrubland => stats.dry_basin += 1,
            GraphBiomeKind::Mangrove
            | GraphBiomeKind::EstuarineCoast
            | GraphBiomeKind::LagoonCoast
            | GraphBiomeKind::RockyCoast
            | GraphBiomeKind::SandyCoast => stats.beach += 1,
            GraphBiomeKind::Desert => stats.desert += 1,
            GraphBiomeKind::Savanna => stats.savanna += 1,
            GraphBiomeKind::TemperateGrassland => stats.grassland += 1,
            GraphBiomeKind::TemperateMixedForest | GraphBiomeKind::TemperateBroadleafForest => {
                stats.temperate_forest += 1
            }
            GraphBiomeKind::BorealForest => stats.boreal_forest += 1,
            GraphBiomeKind::TemperateRainforest
            | GraphBiomeKind::MonsoonForest
            | GraphBiomeKind::TropicalDryForest
            | GraphBiomeKind::TropicalRainforest => stats.rainforest += 1,
            GraphBiomeKind::Tundra | GraphBiomeKind::PolarIce | GraphBiomeKind::PolarBarrens => {
                stats.tundra += 1
            }
            GraphBiomeKind::SubalpineWoodland | GraphBiomeKind::AlpineMeadow => stats.alpine += 1,
        }
    }
    stats
}

fn print_summary(output: &Path, header: &PreviewHeader) {
    println!("wrote {}", output.display());
    println!(
        "seed={} generator_version={} stage={}",
        header.seed, header.generator_version, header.stage
    );
    println!(
        "center=({}, {}) size={}x{} span={} blocks",
        header.center_x, header.center_z, header.width, header.height, header.world_span_blocks
    );
    println!(
        "sites={} ocean={} lake={} wetland={} dry={} beach={} desert={} grassland={} forest={} cold={} mountain={}",
        header.site_count,
        header.stats.ocean,
        header.stats.lake,
        header.stats.wetland,
        header.stats.dry_basin,
        header.stats.beach,
        header.stats.desert,
        header.stats.grassland,
        header.stats.temperate_forest + header.stats.boreal_forest + header.stats.rainforest,
        header.stats.tundra + header.stats.alpine,
        header.stats.mountain,
    );
    println!("metadata: new-world-preview-header iTXt chunk");
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

fn signed_to_unit(value: f32) -> f32 {
    ((value + 1.0) * 0.5).clamp(0.0, 1.0)
}

fn fill_rect(image: &mut RgbImage, x: u32, y: u32, width: u32, height: u32, color: [u8; 3]) {
    let max_x = (x + width).min(image.width());
    let max_y = (y + height).min(image.height());
    for py in y..max_y {
        for px in x..max_x {
            set_pixel(image, px, py, color);
        }
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
        "new-world biome_map_preview".to_string(),
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

fn parse_args() -> Result<PreviewConfig, Box<dyn Error>> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_usage();
        std::process::exit(0);
    }

    let seed = parse_required(&mut args, "seed")?;
    let center_x = parse_required(&mut args, "center-x")?;
    let center_z = parse_required(&mut args, "center-z")?;
    let mut config = PreviewConfig {
        seed,
        center_x,
        center_z,
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
        world_span_blocks: DEFAULT_WORLD_SPAN_BLOCKS,
        region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
        site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
        land_bias: MacroMapConfig::new(seed, WorldMeta::new(seed).generator_version).land_bias,
        stage: DEFAULT_STAGE.to_string(),
        output: None,
    };

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--width" => config.width = parse_option_value(&args, &mut index, "--width")?,
            "--height" => config.height = parse_option_value(&args, &mut index, "--height")?,
            "--world-span-blocks" => {
                config.world_span_blocks =
                    parse_option_value(&args, &mut index, "--world-span-blocks")?;
            }
            "--region-size-blocks" => {
                config.region_size_blocks =
                    parse_option_value(&args, &mut index, "--region-size-blocks")?;
            }
            "--site-spacing-blocks" => {
                config.site_spacing_blocks =
                    parse_option_value(&args, &mut index, "--site-spacing-blocks")?;
            }
            "--land-bias" => {
                config.land_bias = parse_option_value(&args, &mut index, "--land-bias")?;
            }
            "--stage" => config.stage = parse_option_value(&args, &mut index, "--stage")?,
            "--output" => config.output = Some(parse_option_value(&args, &mut index, "--output")?),
            other => return Err(cli_error(format!("unknown argument: {other}"))),
        }
        index += 1;
    }

    Ok(config)
}

fn parse_required<T>(args: &mut Vec<String>, label: &str) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: Error + Send + Sync + 'static,
{
    if args.is_empty() {
        return Err(cli_error(format!("missing required argument: {label}")));
    }
    let value = args.remove(0);
    value
        .parse::<T>()
        .map_err(|err| cli_error(format!("invalid {label} '{value}': {err}")))
}

fn parse_option_value<T>(
    args: &[String],
    index: &mut usize,
    option: &str,
) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: Error + Send + Sync + 'static,
{
    let value_index = index.saturating_add(1);
    let Some(value) = args.get(value_index) else {
        return Err(cli_error(format!("missing value for {option}")));
    };
    *index = value_index;
    value
        .parse::<T>()
        .map_err(|err| cli_error(format!("invalid value for {option}: {err}")))
}

fn print_usage() {
    println!(
        "usage: cargo run --bin biome_map_preview -- <seed> <center-x> <center-z> [options]\n\
         options:\n\
         \t--width <u32>                  default {DEFAULT_WIDTH}\n\
         \t--height <u32>                 default {DEFAULT_HEIGHT}\n\
         \t--world-span-blocks <i32>      default {DEFAULT_WORLD_SPAN_BLOCKS}\n\
         \t--region-size-blocks <i32>     default DEFAULT_GRAPH_REGION_SIZE_BLOCKS\n\
         \t--site-spacing-blocks <i32>    default DEFAULT_SITE_SPACING_BLOCKS\n\
         \t--land-bias <f32>              forwarded to MacroMapConfig\n\
         \t--stage biome_map\n\
         \t--output <path>"
    );
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(std::io::Error::new(ErrorKind::InvalidInput, message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> PreviewConfig {
        PreviewConfig {
            seed: 42,
            center_x: -10,
            center_z: 20,
            width: 640,
            height: 360,
            world_span_blocks: DEFAULT_WORLD_SPAN_BLOCKS,
            region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
            land_bias: MacroMapConfig::new(42, 11).land_bias,
            stage: DEFAULT_STAGE.to_string(),
            output: None,
        }
    }

    #[test]
    fn default_output_path_is_short() {
        let path = test_config()
            .default_output_path()
            .display()
            .to_string()
            .replace('\\', "/");

        assert!(path.ends_with("target/biome-map-preview/s42_x-10_z20.png"));
    }

    #[test]
    fn output_directory_appends_short_name() {
        let mut config = test_config();
        config.output = Some(PathBuf::from("target/biome-map-preview/smoke"));

        let path = output_path_for_config(&config)
            .display()
            .to_string()
            .replace('\\', "/");

        assert!(path.ends_with("target/biome-map-preview/smoke/s42_x-10_z20.png"));
    }

    #[test]
    fn explicit_png_output_path_is_preserved() {
        let mut config = test_config();
        config.output = Some(PathBuf::from("target/custom/biome.png"));

        assert_eq!(
            output_path_for_config(&config),
            PathBuf::from("target/custom/biome.png")
        );
    }

    #[test]
    fn preview_header_records_real_biome_api_shape() {
        let header = PreviewHeader {
            seed: 42,
            generator_version: 11,
            stage: DEFAULT_STAGE.to_string(),
            center_x: 0,
            center_z: 0,
            width: 640,
            height: 360,
            world_span_blocks: DEFAULT_WORLD_SPAN_BLOCKS,
            region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
            land_bias: 0.0,
            graph_area: GraphRegionArea::new(
                GraphRegionCoord::new(0, 0),
                GraphRegionCoord::new(0, 0),
            )
            .unwrap(),
            site_count: 12,
            stats: PreviewBiomeStats {
                ocean: 1,
                lake: 1,
                grassland: 2,
                ..PreviewBiomeStats::default()
            },
            biome_source: "world_generation_macro_map_biomes",
        };

        let metadata = header.to_metadata_text();

        assert!(metadata.contains("binary=biome_map_preview"));
        assert!(metadata.contains("biome_ocean_count=1"));
        assert!(metadata.contains("biome_grassland_count=2"));
        assert!(metadata.contains("GraphBiomeCell,GraphBiomeKind,classify_graph_biome"));
        assert!(metadata.contains("GraphMacroMap.biomes"));
    }

    #[test]
    fn graph_derived_biome_map_is_deterministic() {
        let meta = WorldMeta::new(42);
        let config = test_config();
        let area = config
            .window()
            .graph_area(config.region_size_blocks)
            .unwrap();

        let left = build_biome_map_for_preview(&meta, &config, area).unwrap();
        let right = build_biome_map_for_preview(&meta, &config, area).unwrap();

        assert_eq!(left.patch.sites, right.patch.sites);
        assert_eq!(left.biome_sites, right.biome_sites);
    }

    #[test]
    fn palette_covers_every_graph_biome_kind_with_distinct_colors() {
        let expected = [
            GraphBiomeKind::ShallowOcean,
            GraphBiomeKind::DeepOcean,
            GraphBiomeKind::Mangrove,
            GraphBiomeKind::EstuarineCoast,
            GraphBiomeKind::LagoonCoast,
            GraphBiomeKind::RockyCoast,
            GraphBiomeKind::SandyCoast,
            GraphBiomeKind::Lake,
            GraphBiomeKind::Marsh,
            GraphBiomeKind::Swamp,
            GraphBiomeKind::FloodedForest,
            GraphBiomeKind::Desert,
            GraphBiomeKind::SemiDesert,
            GraphBiomeKind::Steppe,
            GraphBiomeKind::DryShrubland,
            GraphBiomeKind::MediterraneanShrubland,
            GraphBiomeKind::PolarIce,
            GraphBiomeKind::PolarBarrens,
            GraphBiomeKind::Tundra,
            GraphBiomeKind::SubalpineWoodland,
            GraphBiomeKind::AlpineMeadow,
            GraphBiomeKind::BorealForest,
            GraphBiomeKind::TropicalRainforest,
            GraphBiomeKind::MonsoonForest,
            GraphBiomeKind::TropicalDryForest,
            GraphBiomeKind::Savanna,
            GraphBiomeKind::TemperateRainforest,
            GraphBiomeKind::TemperateMixedForest,
            GraphBiomeKind::TemperateBroadleafForest,
            GraphBiomeKind::TemperateGrassland,
        ];

        assert_eq!(BIOME_PALETTE.len(), expected.len());
        for biome in expected {
            let matches = BIOME_PALETTE
                .iter()
                .filter(|entry| entry.biome == biome)
                .count();
            assert_eq!(
                matches, 1,
                "{biome:?} should have exactly one palette entry"
            );
        }
        for (left_index, left) in BIOME_PALETTE.iter().enumerate() {
            for right in BIOME_PALETTE.iter().skip(left_index + 1) {
                assert_ne!(left.color, right.color);
                assert!(
                    color_distance_sq(left.color, right.color) >= 900,
                    "{:?} and {:?} colors are too similar",
                    left.biome,
                    right.biome
                );
            }
        }
    }

    fn color_distance_sq(left: [u8; 3], right: [u8; 3]) -> i32 {
        left.iter()
            .zip(right)
            .map(|(left, right)| {
                let delta = *left as i32 - right as i32;
                delta * delta
            })
            .sum()
    }
}
