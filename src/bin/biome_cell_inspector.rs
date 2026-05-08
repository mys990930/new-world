use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use new_world::world::WorldMeta;
use new_world::world::generation::{
    BoundaryCache, BoundaryConfig, DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS,
    GraphBiomeCell, GraphBiomeKind, GraphBiomeWaterRole, GraphDrainageNodeKind,
    GraphHydrologyGraph, GraphRegionArea, GraphRegionCoord, GraphRiverSegment, HydrologyConfig,
    MacroEdge, MacroLakeEdgeClass, MacroMapConfig, MacroSite, MacroSurfaceKind, VoronoiCornerId,
    VoronoiGraphConfig, VoronoiGraphPatch, VoronoiGraphPatchRequest, VoronoiSiteId,
    WorldPlanePoint, generate_macro_map, generate_noisy_boundaries, generate_voronoi_graph_patch,
    graph_region_for_world_block, solve_hydrology,
};

const DEFAULT_WIDTH: u32 = 1400;
const DEFAULT_HEIGHT: u32 = 900;
const DEFAULT_WORLD_SPAN_BLOCKS: i32 = 32768;
const DEFAULT_STAGE: &str = "biome_cell_inspector";
const OUTPUT_DIR: &str = "target/biome-cell-inspector";
const BIOME_PALETTE: [BiomePaletteEntry; 30] = [
    BiomePaletteEntry::new(GraphBiomeKind::DeepOcean, "DeepOcean", [0, 32, 96]),
    BiomePaletteEntry::new(GraphBiomeKind::ShallowOcean, "ShallowOcean", [0, 147, 196]),
    BiomePaletteEntry::new(GraphBiomeKind::Mangrove, "Mangrove", [0, 86, 63]),
    BiomePaletteEntry::new(
        GraphBiomeKind::EstuarineCoast,
        "EstuarineCoast",
        [96, 171, 130],
    ),
    BiomePaletteEntry::new(GraphBiomeKind::LagoonCoast, "LagoonCoast", [85, 210, 198]),
    BiomePaletteEntry::new(GraphBiomeKind::RockyCoast, "RockyCoast", [115, 118, 130]),
    BiomePaletteEntry::new(GraphBiomeKind::SandyCoast, "SandyCoast", [238, 213, 132]),
    BiomePaletteEntry::new(GraphBiomeKind::Lake, "Lake", [52, 88, 209]),
    BiomePaletteEntry::new(GraphBiomeKind::Marsh, "Marsh", [116, 150, 110]),
    BiomePaletteEntry::new(GraphBiomeKind::Swamp, "Swamp", [57, 69, 42]),
    BiomePaletteEntry::new(
        GraphBiomeKind::FloodedForest,
        "FloodedForest",
        [32, 78, 136],
    ),
    BiomePaletteEntry::new(GraphBiomeKind::Desert, "Desert", [224, 173, 43]),
    BiomePaletteEntry::new(GraphBiomeKind::SemiDesert, "SemiDesert", [190, 119, 57]),
    BiomePaletteEntry::new(GraphBiomeKind::Steppe, "Steppe", [154, 178, 105]),
    BiomePaletteEntry::new(GraphBiomeKind::DryShrubland, "DryShrubland", [136, 88, 52]),
    BiomePaletteEntry::new(
        GraphBiomeKind::MediterraneanShrubland,
        "MediterraneanShrubland",
        [104, 116, 38],
    ),
    BiomePaletteEntry::new(GraphBiomeKind::PolarIce, "PolarIce", [232, 245, 250]),
    BiomePaletteEntry::new(
        GraphBiomeKind::PolarBarrens,
        "PolarBarrens",
        [188, 188, 188],
    ),
    BiomePaletteEntry::new(GraphBiomeKind::Tundra, "Tundra", [169, 178, 153]),
    BiomePaletteEntry::new(
        GraphBiomeKind::SubalpineWoodland,
        "SubalpineWoodland",
        [52, 106, 94],
    ),
    BiomePaletteEntry::new(
        GraphBiomeKind::AlpineMeadow,
        "AlpineMeadow",
        [144, 128, 166],
    ),
    BiomePaletteEntry::new(GraphBiomeKind::BorealForest, "BorealForest", [24, 80, 98]),
    BiomePaletteEntry::new(
        GraphBiomeKind::TropicalRainforest,
        "TropicalRainforest",
        [0, 116, 54],
    ),
    BiomePaletteEntry::new(
        GraphBiomeKind::MonsoonForest,
        "MonsoonForest",
        [38, 156, 69],
    ),
    BiomePaletteEntry::new(
        GraphBiomeKind::TropicalDryForest,
        "TropicalDryForest",
        [108, 162, 39],
    ),
    BiomePaletteEntry::new(GraphBiomeKind::Savanna, "Savanna", [201, 190, 55]),
    BiomePaletteEntry::new(
        GraphBiomeKind::TemperateRainforest,
        "TemperateRainforest",
        [0, 128, 116],
    ),
    BiomePaletteEntry::new(
        GraphBiomeKind::TemperateMixedForest,
        "TemperateMixedForest",
        [48, 132, 47],
    ),
    BiomePaletteEntry::new(
        GraphBiomeKind::TemperateBroadleafForest,
        "TemperateBroadleafForest",
        [81, 154, 64],
    ),
    BiomePaletteEntry::new(
        GraphBiomeKind::TemperateGrassland,
        "TemperateGrassland",
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
            "{OUTPUT_DIR}/s{}_x{}_z{}.html",
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

    fn world_to_screen(self, point: WorldPlanePoint) -> (f32, f32) {
        (
            (point.x - self.min_x()) / self.world_span_x * self.width as f32,
            (point.z - self.min_z()) / self.world_span_z * self.height as f32,
        )
    }

    fn contains_world_point(self, point: WorldPlanePoint) -> bool {
        point.x >= self.min_x()
            && point.x <= self.max_x()
            && point.z >= self.min_z()
            && point.z <= self.max_z()
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
        GraphRegionArea::new(min, max).ok_or_else(|| cli_error("invalid biome cell inspector area"))
    }
}

#[derive(Debug, Clone)]
struct InspectorGraph {
    patch: VoronoiGraphPatch,
    macro_sites: HashMap<VoronoiSiteId, MacroSite>,
    biome_sites: HashMap<VoronoiSiteId, GraphBiomeCell>,
    macro_edges: Vec<MacroEdge>,
    hydrology: GraphHydrologyGraph,
    boundary: BoundaryCache,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct SiteOverlayStats {
    lake_boundary_edges: usize,
    lake_internal_edges: usize,
    lake_adjacent_edges: usize,
    river_segments: usize,
    max_river_flow: f32,
    lake_inlet_markers: usize,
    lake_outlet_markers: usize,
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?.validate()?;
    let meta = WorldMeta::new(config.seed);
    let window = config.window();
    let graph_area = window.graph_area(config.region_size_blocks)?;
    let graph = build_inspector_graph(&meta, &config, graph_area)?;
    let html = render_html(&config, window, graph_area, &graph)?;
    let output = output_path_for_config(&config);
    write_html(&output, &html)?;

    println!("wrote {}", output.display());
    println!(
        "seed={} stage={} cells={} rivers={} lake_edges={}",
        meta.seed,
        config.stage,
        graph.biome_sites.len(),
        graph.hydrology.segments.len(),
        graph
            .macro_edges
            .iter()
            .filter(|edge| edge.lake_class.excludes_selected_river())
            .count()
    );

    Ok(())
}

fn build_inspector_graph(
    meta: &WorldMeta,
    config: &PreviewConfig,
    graph_area: GraphRegionArea,
) -> Result<InspectorGraph, Box<dyn Error>> {
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
    let macro_map = generate_macro_map(&patch, macro_map_config_for_preview(meta, config));
    let hydrology = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());
    let boundary = generate_noisy_boundaries(
        &patch,
        &macro_map,
        BoundaryConfig::new(meta.seed, meta.generator_version),
    );
    let macro_sites = macro_map
        .sites
        .iter()
        .map(|site| (site.id, *site))
        .collect::<HashMap<_, _>>();
    let biome_sites = macro_map
        .biomes
        .iter()
        .map(|site| (site.site, *site))
        .collect::<HashMap<_, _>>();

    Ok(InspectorGraph {
        patch,
        macro_sites,
        biome_sites,
        macro_edges: macro_map.edges,
        hydrology,
        boundary,
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
        .map_err(|_| cli_error("biome cell inspector padding overflowed"))
}

fn macro_map_config_for_preview(meta: &WorldMeta, config: &PreviewConfig) -> MacroMapConfig {
    MacroMapConfig {
        land_bias: config.land_bias,
        ..MacroMapConfig::new(meta.seed, meta.generator_version)
    }
}

fn render_html(
    config: &PreviewConfig,
    window: PreviewWindow,
    graph_area: GraphRegionArea,
    graph: &InspectorGraph,
) -> Result<String, Box<dyn Error>> {
    let corner_positions = graph
        .patch
        .corners
        .iter()
        .map(|corner| (corner.id, corner.position))
        .collect::<HashMap<_, _>>();
    let site_edges = edges_by_site(&graph.macro_edges);
    let river_by_edge = graph
        .hydrology
        .segments
        .iter()
        .map(|segment| (segment.edge, segment))
        .collect::<HashMap<_, _>>();

    let mut cells = String::new();
    let mut details = Vec::new();
    for site in &graph.patch.sites {
        if !window.contains_world_point(site.position) {
            continue;
        }
        let Some(macro_site) = graph.macro_sites.get(&site.id).copied() else {
            continue;
        };
        let Some(biome_site) = graph.biome_sites.get(&site.id).copied() else {
            continue;
        };
        let Some(points) = polygon_points_for_site(
            site.id,
            site.position,
            &site_edges,
            &corner_positions,
            window,
        ) else {
            continue;
        };
        let stats = overlay_stats_for_site(site.id, graph, &site_edges, &river_by_edge);
        let index = details.len();
        let color = hex_color(color_for_biome_site(biome_site, macro_site));
        cells.push_str(&format!(
            "<polygon class=\"cell\" data-index=\"{index}\" points=\"{points}\" fill=\"{color}\"><title>{} / {:?}</title></polygon>\n",
            html_escape(biome_label(biome_site.biome)),
            macro_site.surface_kind
        ));
        details.push(site_detail_json(site.id, macro_site, biome_site, stats));
    }

    let overlays = render_overlays(window, graph);
    let legend = render_palette_legend();
    let details_json = format!("[{}]", details.join(","));

    Ok(format!(
        r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>new-world biome cell inspector</title>
<style>
:root {{ color-scheme: dark; font-family: Inter, Segoe UI, sans-serif; background:#101417; color:#e8ece8; }}
body {{ margin:0; min-height:100vh; display:grid; grid-template-columns:minmax(0,1fr) 360px; }}
main {{ min-width:0; padding:14px; }}
aside {{ border-left:1px solid #2c3439; padding:16px; background:#151b1f; overflow:auto; }}
.map-wrap {{ position:relative; width:100%; height:calc(100vh - 28px); min-height:420px; background:#080d12; overflow:hidden; }}
svg {{ width:100%; height:100%; display:block; }}
.cell {{ stroke:#13191c; stroke-width:0.75; cursor:pointer; transition:filter .12s, stroke-width .12s; }}
.cell:hover, .cell.active {{ filter:brightness(1.22); stroke:#fff5b8; stroke-width:2.2; }}
.river {{ fill:none; stroke:#21b8da; stroke-linecap:round; stroke-linejoin:round; pointer-events:none; }}
.lake-edge {{ fill:none; stroke:#9de8fb; stroke-width:2; stroke-linecap:round; pointer-events:none; opacity:.86; }}
.lake-marker {{ pointer-events:none; }}
h1 {{ font-size:18px; margin:0 0 4px; }}
.meta {{ color:#aab4b7; font-size:12px; margin-bottom:14px; }}
.row {{ display:grid; grid-template-columns:155px 1fr; gap:10px; padding:5px 0; border-bottom:1px solid #263037; font-size:13px; }}
.key {{ color:#aab4b7; }}
.value {{ color:#f3f3e8; overflow-wrap:anywhere; }}
.legend {{ display:grid; grid-template-columns:1fr 1fr; gap:6px 12px; margin-top:16px; }}
.legend div {{ display:flex; align-items:center; gap:7px; font-size:11px; color:#d7ddd8; }}
.swatch {{ width:16px; height:12px; border:1px solid rgba(255,255,255,.22); }}
.hint {{ color:#aab4b7; font-size:12px; line-height:1.45; margin:10px 0 16px; }}
@media (max-width: 900px) {{
  body {{ grid-template-columns:1fr; }}
  aside {{ border-left:0; border-top:1px solid #2c3439; max-height:48vh; }}
  .map-wrap {{ height:56vh; }}
}}
</style>
</head>
<body>
<main>
<div class="map-wrap">
<svg id="map" viewBox="0 0 {width} {height}" role="img" aria-label="Biome cell inspector map">
<rect width="{width}" height="{height}" fill="#081018"/>
{cells}
{overlays}
<text x="18" y="28" fill="#edf2dc" font-size="16" font-weight="700">N</text>
<text x="18" y="48" fill="#9fb0b5" font-size="12">E right</text>
</svg>
</div>
</main>
<aside>
<h1>Biome Cell Inspector</h1>
<div class="meta">seed={seed} center=({center_x}, {center_z}) span={span} graph=({min_rx},{min_rz})..({max_rx},{max_rz})</div>
<p class="hint">Hover or click a Voronoi cell. Fill color is the resolved biome; cyan strokes are selected rivers; pale cyan strokes mark macro lake edges.</p>
<div id="details"></div>
<h1 style="margin-top:18px">Legend</h1>
<div class="legend">{legend}</div>
</aside>
<script>
const CELL_DETAILS = {details_json};
const details = document.getElementById('details');
let active = null;
function showCell(index) {{
  const data = CELL_DETAILS[index];
  if (!data) return;
  details.innerHTML = Object.entries(data).map(([key, value]) =>
    `<div class="row"><div class="key">${{key}}</div><div class="value">${{value}}</div></div>`
  ).join('');
}}
document.querySelectorAll('.cell').forEach(cell => {{
  const index = Number(cell.dataset.index);
  cell.addEventListener('mouseenter', () => showCell(index));
  cell.addEventListener('click', () => {{
    if (active) active.classList.remove('active');
    active = cell;
    cell.classList.add('active');
    showCell(index);
  }});
}});
showCell(0);
</script>
</body>
</html>
"##,
        width = window.width,
        height = window.height,
        cells = cells,
        overlays = overlays,
        legend = legend,
        details_json = details_json,
        seed = config.seed,
        center_x = config.center_x,
        center_z = config.center_z,
        span = config.world_span_blocks,
        min_rx = graph_area.min.x,
        min_rz = graph_area.min.z,
        max_rx = graph_area.max.x,
        max_rz = graph_area.max.z
    ))
}

fn edges_by_site(edges: &[MacroEdge]) -> HashMap<VoronoiSiteId, Vec<MacroEdge>> {
    let mut by_site = HashMap::<VoronoiSiteId, Vec<MacroEdge>>::new();
    for edge in edges {
        by_site.entry(edge.sites[0]).or_default().push(*edge);
        by_site.entry(edge.sites[1]).or_default().push(*edge);
    }
    by_site
}

fn polygon_points_for_site(
    site: VoronoiSiteId,
    site_position: WorldPlanePoint,
    site_edges: &HashMap<VoronoiSiteId, Vec<MacroEdge>>,
    corners: &HashMap<VoronoiCornerId, WorldPlanePoint>,
    window: PreviewWindow,
) -> Option<String> {
    let mut points = Vec::<WorldPlanePoint>::new();
    for edge in site_edges.get(&site)? {
        for corner in edge.corners {
            let Some(point) = corners.get(&corner).copied() else {
                continue;
            };
            if !points.iter().any(|existing| {
                (existing.x - point.x).abs() <= f32::EPSILON
                    && (existing.z - point.z).abs() <= f32::EPSILON
            }) {
                points.push(point);
            }
        }
    }
    if points.len() < 3 {
        return None;
    }
    points.sort_by(|left, right| {
        let left_angle = (left.z - site_position.z).atan2(left.x - site_position.x);
        let right_angle = (right.z - site_position.z).atan2(right.x - site_position.x);
        left_angle.total_cmp(&right_angle)
    });
    Some(
        points
            .into_iter()
            .map(|point| {
                let (x, y) = window.world_to_screen(point);
                format!("{x:.2},{y:.2}")
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}

fn overlay_stats_for_site(
    site: VoronoiSiteId,
    graph: &InspectorGraph,
    site_edges: &HashMap<VoronoiSiteId, Vec<MacroEdge>>,
    river_by_edge: &HashMap<new_world::world::generation::VoronoiEdgeId, &GraphRiverSegment>,
) -> SiteOverlayStats {
    let mut stats = SiteOverlayStats::default();
    if let Some(edges) = site_edges.get(&site) {
        for edge in edges {
            match edge.lake_class {
                MacroLakeEdgeClass::NonLake => {}
                MacroLakeEdgeClass::LakeAdjacentLand => stats.lake_adjacent_edges += 1,
                MacroLakeEdgeClass::LakeBoundary => stats.lake_boundary_edges += 1,
                MacroLakeEdgeClass::LakeInternal => stats.lake_internal_edges += 1,
            }
            if let Some(segment) = river_by_edge.get(&edge.id) {
                stats.river_segments += 1;
                stats.max_river_flow = stats.max_river_flow.max(segment.flow_accumulation);
            }
        }
    }

    for node in &graph.hydrology.nodes {
        match node.kind {
            GraphDrainageNodeKind::LakeInlet => {
                if node_is_near_site_edges(node.corner, site, site_edges) {
                    stats.lake_inlet_markers += 1;
                }
            }
            GraphDrainageNodeKind::LakeOutlet => {
                if node_is_near_site_edges(node.corner, site, site_edges) {
                    stats.lake_outlet_markers += 1;
                }
            }
            _ => {}
        }
    }
    stats
}

fn node_is_near_site_edges(
    corner: VoronoiCornerId,
    site: VoronoiSiteId,
    site_edges: &HashMap<VoronoiSiteId, Vec<MacroEdge>>,
) -> bool {
    site_edges
        .get(&site)
        .is_some_and(|edges| edges.iter().any(|edge| edge.corners.contains(&corner)))
}

fn render_overlays(window: PreviewWindow, graph: &InspectorGraph) -> String {
    let mut svg = String::new();
    for curve in &graph.boundary.curves {
        if let Some(edge) = graph.macro_edges.iter().find(|edge| edge.id == curve.edge) {
            if edge.lake_class.excludes_selected_river() {
                svg.push_str(&format!(
                    "<polyline class=\"lake-edge\" points=\"{}\" />\n",
                    curve_points(window, &curve.points)
                ));
            }
        }
    }
    for segment in &graph.hydrology.segments {
        let Some(curve) = graph.boundary.curve_for_edge(segment.edge) else {
            continue;
        };
        let width = river_width(segment.flow_accumulation);
        svg.push_str(&format!(
            "<polyline class=\"river\" points=\"{}\" stroke-width=\"{width}\" opacity=\"{:.2}\" />\n",
            curve_points(window, &curve.points),
            river_amount(segment.flow_accumulation)
        ));
    }
    for node in &graph.hydrology.nodes {
        let Some((color, radius)) = hydrology_marker_style(node.kind) else {
            continue;
        };
        let (x, y) = window.world_to_screen(node.position);
        svg.push_str(&format!(
            "<circle class=\"lake-marker\" cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{radius}\" fill=\"{}\" stroke=\"#061018\" stroke-width=\"1.5\" />\n",
            hex_color(color)
        ));
    }
    svg
}

fn curve_points(window: PreviewWindow, points: &[WorldPlanePoint]) -> String {
    points
        .iter()
        .map(|point| {
            let (x, y) = window.world_to_screen(*point);
            format!("{x:.2},{y:.2}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn hydrology_marker_style(kind: GraphDrainageNodeKind) -> Option<([u8; 3], f32)> {
    match kind {
        GraphDrainageNodeKind::LakeInlet => Some(([252, 224, 66], 3.0)),
        GraphDrainageNodeKind::LakeOutlet => Some(([62, 113, 255], 3.6)),
        GraphDrainageNodeKind::Sink => Some(([128, 75, 178], 3.0)),
        GraphDrainageNodeKind::CoastOutlet => Some(([20, 246, 184], 2.8)),
        _ => None,
    }
}

fn river_amount(flow: f32) -> f32 {
    (0.62 + flow.sqrt() * 0.035).clamp(0.68, 0.98)
}

fn river_width(flow: f32) -> i32 {
    if flow >= 80.0 {
        7
    } else if flow >= 36.0 {
        5
    } else if flow >= 18.0 {
        4
    } else {
        3
    }
}

fn site_detail_json(
    id: VoronoiSiteId,
    macro_site: MacroSite,
    biome_site: GraphBiomeCell,
    overlay: SiteOverlayStats,
) -> String {
    let context = biome_site.context;
    let fields = [
        ("site", id.0.to_string()),
        ("biome", biome_label(biome_site.biome).to_string()),
        ("macro_surface", format!("{:?}", macro_site.surface_kind)),
        (
            "water_role",
            water_role_label(context.water_role).to_string(),
        ),
        ("temperature", format!("{:.3}", context.temperature)),
        ("hydration", format!("{:.3}", context.hydration)),
        ("elevation", format!("{:.3}", context.elevation)),
        ("continentality", format!("{:.3}", context.continentality)),
        ("coastness", format!("{:.3}", context.coastness)),
        ("mountainness", format!("{:.3}", context.mountainness)),
        ("ruggedness", format!("{:.3}", context.ruggedness)),
        (
            "macro_site_kind",
            macro_site_kind(macro_site.surface_kind).to_string(),
        ),
        (
            "macro_elevation",
            format!("{:.3}", macro_site.signed_macro_elevation),
        ),
        (
            "distance_to_coast",
            format!("{:.1}", macro_site.distance_to_coast_blocks),
        ),
        (
            "lake_boundary_edges",
            overlay.lake_boundary_edges.to_string(),
        ),
        (
            "lake_internal_edges",
            overlay.lake_internal_edges.to_string(),
        ),
        (
            "lake_adjacent_edges",
            overlay.lake_adjacent_edges.to_string(),
        ),
        ("river_segments", overlay.river_segments.to_string()),
        ("max_river_flow", format!("{:.3}", overlay.max_river_flow)),
        ("lake_inlet_markers", overlay.lake_inlet_markers.to_string()),
        (
            "lake_outlet_markers",
            overlay.lake_outlet_markers.to_string(),
        ),
    ];
    format!(
        "{{{}}}",
        fields
            .into_iter()
            .map(|(key, value)| format!("\"{}\":\"{}\"", json_escape(key), json_escape(&value)))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn color_for_biome_site(site: GraphBiomeCell, macro_site: MacroSite) -> [u8; 3] {
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
    let elevation = ((macro_site.signed_macro_elevation + 1.0) * 0.5).clamp(0.0, 1.0);
    blend(base, [248, 249, 242], (elevation - 0.72).max(0.0) * 1.25)
}

fn color_for_biome_kind(biome: GraphBiomeKind) -> [u8; 3] {
    BIOME_PALETTE
        .iter()
        .find(|entry| entry.biome == biome)
        .map(|entry| entry.color)
        .expect("palette should contain every GraphBiomeKind")
}

fn biome_label(biome: GraphBiomeKind) -> &'static str {
    BIOME_PALETTE
        .iter()
        .find(|entry| entry.biome == biome)
        .map(|entry| entry.label)
        .expect("palette should contain every GraphBiomeKind")
}

fn water_role_label(role: GraphBiomeWaterRole) -> &'static str {
    match role {
        GraphBiomeWaterRole::Land => "Land",
        GraphBiomeWaterRole::Coast => "Coast",
        GraphBiomeWaterRole::ShallowOcean => "ShallowOcean",
        GraphBiomeWaterRole::DeepOcean => "DeepOcean",
        GraphBiomeWaterRole::Lake => "Lake",
        GraphBiomeWaterRole::Wetland => "Wetland",
        GraphBiomeWaterRole::DryBasin => "DryBasin",
    }
}

fn macro_site_kind(kind: MacroSurfaceKind) -> &'static str {
    match kind {
        MacroSurfaceKind::Continent => "land continent",
        MacroSurfaceKind::Island => "land island",
        MacroSurfaceKind::OceanBasin => "ocean basin",
        MacroSurfaceKind::CoastLand => "land coast",
        MacroSurfaceKind::CoastIsland => "island coast",
        MacroSurfaceKind::CoastOcean => "ocean coast",
        MacroSurfaceKind::DryBasin => "dry closed basin",
        MacroSurfaceKind::LakeCandidate => "macro lake",
        MacroSurfaceKind::WetlandCandidate => "macro wetland",
    }
}

fn render_palette_legend() -> String {
    BIOME_PALETTE
        .iter()
        .map(|entry| {
            format!(
                "<div><span class=\"swatch\" style=\"background:{}\"></span>{}</div>",
                hex_color(entry.color),
                html_escape(entry.label)
            )
        })
        .collect::<Vec<_>>()
        .join("")
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

fn hex_color(color: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2])
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn write_html(output: &Path, html: &str) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, html)?;
    Ok(())
}

fn output_path_for_config(config: &PreviewConfig) -> PathBuf {
    config.output.as_ref().map_or_else(
        || config.default_output_path(),
        |path| {
            if looks_like_file(path) {
                path.clone()
            } else {
                path.join(format!(
                    "s{}_x{}_z{}.html",
                    config.seed, config.center_x, config.center_z
                ))
            }
        },
    )
}

fn looks_like_file(path: &Path) -> bool {
    path.extension().is_some()
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
        "usage: cargo run --bin biome_cell_inspector -- <seed> <center-x> <center-z> [options]\n\
         options:\n\
         \t--width <u32>                  default {DEFAULT_WIDTH}\n\
         \t--height <u32>                 default {DEFAULT_HEIGHT}\n\
         \t--world-span-blocks <i32>      default {DEFAULT_WORLD_SPAN_BLOCKS}\n\
         \t--region-size-blocks <i32>     default DEFAULT_GRAPH_REGION_SIZE_BLOCKS\n\
         \t--site-spacing-blocks <i32>    default DEFAULT_SITE_SPACING_BLOCKS\n\
         \t--land-bias <f32>              forwarded to MacroMapConfig\n\
         \t--stage biome_cell_inspector\n\
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
    fn default_output_path_is_html() {
        let path = test_config()
            .default_output_path()
            .display()
            .to_string()
            .replace('\\', "/");

        assert!(path.ends_with("target/biome-cell-inspector/s42_x-10_z20.html"));
    }

    #[test]
    fn output_directory_appends_html_name() {
        let mut config = test_config();
        config.output = Some(PathBuf::from("target/biome-cell-inspector/smoke"));

        let path = output_path_for_config(&config)
            .display()
            .to_string()
            .replace('\\', "/");

        assert!(path.ends_with("target/biome-cell-inspector/smoke/s42_x-10_z20.html"));
    }

    #[test]
    fn explicit_html_output_path_is_preserved() {
        let mut config = test_config();
        config.output = Some(PathBuf::from("target/custom/inspector.html"));

        assert_eq!(
            output_path_for_config(&config),
            PathBuf::from("target/custom/inspector.html")
        );
    }

    #[test]
    fn generated_html_exposes_interactive_cell_fields() {
        let meta = WorldMeta::new(42);
        let config = PreviewConfig {
            width: 320,
            height: 180,
            world_span_blocks: 8192,
            ..test_config()
        };
        let window = config.window();
        let area = window.graph_area(config.region_size_blocks).unwrap();
        let graph = build_inspector_graph(&meta, &config, area).unwrap();
        let html = render_html(&config, window, area, &graph).unwrap();

        assert!(html.contains("CELL_DETAILS"));
        assert!(html.contains("class=\"cell\""));
        assert!(html.contains("macro_surface"));
        assert!(html.contains("water_role"));
        assert!(html.contains("temperature"));
        assert!(html.contains("hydration"));
        assert!(html.contains("continentality"));
        assert!(html.contains("ruggedness"));
        assert!(html.contains("river_segments"));
        assert!(html.contains("lake_boundary_edges"));
        assert!(html.contains("class=\"river\""));
    }

    #[test]
    fn palette_covers_expected_graph_biome_kinds() {
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
            assert_eq!(
                BIOME_PALETTE
                    .iter()
                    .filter(|entry| entry.biome == biome)
                    .count(),
                1
            );
        }
    }
}
