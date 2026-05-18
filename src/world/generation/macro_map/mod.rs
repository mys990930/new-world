use rayon::prelude::*;
use std::collections::{HashMap, VecDeque};

use super::biome::{classify_graph_biome, GraphBiomeCell, GraphBiomeContext, GraphBiomeWaterRole};
use super::graph::{
    GraphBaseFields, GraphRegionCoord, VoronoiCornerId, VoronoiEdgeId, VoronoiGraphPatch,
    VoronoiSiteId, WorldPlanePoint,
};

pub const DEFAULT_MACRO_COAST_WIDTH_BLOCKS: f32 = 384.0;
pub const DEFAULT_MACRO_RIDGE_CANDIDATE_THRESHOLD: f32 = 0.28;
pub const DEFAULT_MACRO_RIVER_CANDIDATE_THRESHOLD: f32 = 0.66;
pub const DEFAULT_MACRO_LAND_BIAS: f32 = 0.14;

const LAND_COMPONENT_NAMESPACE: u64 = 0x4f1d_77a9_b384_d13e;
const OCEAN_COMPONENT_NAMESPACE: u64 = 0x9a72_c80d_31ef_624b;
const SMALL_STREAM_POCKET_LAKE_NAMESPACE: u64 = 0x3c2d_8f19_641a_b057;
const TINY_LOCAL_MINIMA_LAKE_NAMESPACE: u64 = 0x85e5_3c3f_51ef_9c2a;
const DEFAULT_MACRO_GRAPH_DISTANCE_STEP_BLOCKS: f32 = 192.0;
pub const DEFAULT_TINY_LOCAL_MINIMA_LAKE_CHANCE_PER_10K: u32 = 3_600;
pub const DEFAULT_TINY_LOCAL_MINIMA_LAKE_MAX_CHANCE_PER_10K: u32 = 4_000;
pub const DEFAULT_TINY_LOCAL_MINIMA_LAKE_MAX_CELLS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MacroMapConfig {
    pub seed: u64,
    pub generator_version: u32,
    pub sea_level: f32,
    pub coast_width_blocks: f32,
    pub ridge_candidate_threshold: f32,
    pub river_candidate_threshold: f32,
    pub land_bias: f32,
}

impl MacroMapConfig {
    pub const fn new(seed: u64, generator_version: u32) -> Self {
        Self {
            seed,
            generator_version,
            sea_level: 0.0,
            coast_width_blocks: DEFAULT_MACRO_COAST_WIDTH_BLOCKS,
            ridge_candidate_threshold: DEFAULT_MACRO_RIDGE_CANDIDATE_THRESHOLD,
            river_candidate_threshold: DEFAULT_MACRO_RIVER_CANDIDATE_THRESHOLD,
            land_bias: DEFAULT_MACRO_LAND_BIAS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacroContinentId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacroOceanBasinId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroSurfaceKind {
    Continent,
    Island,
    OceanBasin,
    CoastLand,
    CoastIsland,
    CoastOcean,
    DryBasin,
    LakeCandidate,
    WetlandCandidate,
}

impl MacroSurfaceKind {
    pub const fn is_land_owned(self) -> bool {
        matches!(
            self,
            Self::Continent
                | Self::Island
                | Self::CoastLand
                | Self::CoastIsland
                | Self::DryBasin
                | Self::LakeCandidate
                | Self::WetlandCandidate
        )
    }

    pub const fn is_ocean_owned(self) -> bool {
        matches!(self, Self::OceanBasin | Self::CoastOcean)
    }

    pub const fn is_coast(self) -> bool {
        matches!(self, Self::CoastLand | Self::CoastIsland | Self::CoastOcean)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroLakeEdgeClass {
    NonLake,
    LakeAdjacentLand,
    LakeBoundary,
    LakeInternal,
}

impl MacroLakeEdgeClass {
    pub const fn excludes_selected_river(self) -> bool {
        !matches!(self, Self::NonLake)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MacroSite {
    pub id: VoronoiSiteId,
    pub owner_region: GraphRegionCoord,
    pub position: WorldPlanePoint,
    pub surface_kind: MacroSurfaceKind,
    pub continent: Option<MacroContinentId>,
    pub ocean_basin: Option<MacroOceanBasinId>,
    pub signed_macro_elevation: f32,
    pub continentality: f32,
    pub coastness: f32,
    pub distance_to_coast_blocks: f32,
    pub distance_to_continent_core_blocks: f32,
    pub distance_to_ocean_basin_blocks: f32,
    pub mountainness: f32,
    pub ridgeness: f32,
    pub basinness: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MacroCorner {
    pub id: VoronoiCornerId,
    pub position: WorldPlanePoint,
    pub surface_kind: MacroSurfaceKind,
    pub continent: Option<MacroContinentId>,
    pub ocean_basin: Option<MacroOceanBasinId>,
    pub signed_macro_elevation: f32,
    pub continentality: f32,
    pub coastness: f32,
    pub distance_to_coast_blocks: f32,
    pub mountainness: f32,
    pub ridgeness: f32,
    pub basinness: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MacroEdgeGuide {
    pub is_coast: bool,
    pub is_ridge_candidate: bool,
    pub is_river_candidate: bool,
    pub is_fault_candidate: bool,
    pub coastness: f32,
    pub mountainness: f32,
    pub ridgeness: f32,
    pub signed_elevation_gradient: f32,
    pub drainage_divide_potential: f32,
    pub river_potential: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MacroEdge {
    pub id: VoronoiEdgeId,
    pub sites: [VoronoiSiteId; 2],
    pub corners: [VoronoiCornerId; 2],
    pub guide: MacroEdgeGuide,
    pub lake_class: MacroLakeEdgeClass,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GraphMacroMap {
    pub sites: Vec<MacroSite>,
    pub corners: Vec<MacroCorner>,
    pub edges: Vec<MacroEdge>,
    pub biomes: Vec<GraphBiomeCell>,
}

impl GraphMacroMap {
    pub fn is_empty(&self) -> bool {
        self.sites.is_empty() && self.corners.is_empty() && self.edges.is_empty()
    }

    pub fn site(&self, id: VoronoiSiteId) -> Option<&MacroSite> {
        self.sites.iter().find(|site| site.id == id)
    }

    pub fn corner(&self, id: VoronoiCornerId) -> Option<&MacroCorner> {
        self.corners.iter().find(|corner| corner.id == id)
    }

    pub fn edge(&self, id: VoronoiEdgeId) -> Option<&MacroEdge> {
        self.edges.iter().find(|edge| edge.id == id)
    }

    pub fn biome(&self, site: VoronoiSiteId) -> Option<&GraphBiomeCell> {
        self.biomes.iter().find(|biome| biome.site == site)
    }

    pub fn coast_edges(&self) -> impl Iterator<Item = &MacroEdge> {
        self.edges.iter().filter(|edge| edge.guide.is_coast)
    }

    pub fn ridge_candidate_edges(&self) -> impl Iterator<Item = &MacroEdge> {
        self.edges
            .iter()
            .filter(|edge| edge.guide.is_ridge_candidate)
    }

    pub fn fault_candidate_edges(&self) -> impl Iterator<Item = &MacroEdge> {
        self.edges
            .iter()
            .filter(|edge| edge.guide.is_fault_candidate)
    }

    pub fn river_candidate_edges(&self) -> impl Iterator<Item = &MacroEdge> {
        self.edges
            .iter()
            .filter(|edge| edge.guide.is_river_candidate)
    }
}

pub fn generate_macro_map(patch: &VoronoiGraphPatch, config: MacroMapConfig) -> GraphMacroMap {
    validate_macro_map_config(config);

    let site_context = resolve_site_context(patch, config);
    let mut site_builds = patch
        .sites
        .par_iter()
        .enumerate()
        .map(|(index, site)| {
            let context = site_context[index];
            macro_site_from_context(site.id, site.owner_region, site.position, context, config)
        })
        .collect::<Vec<_>>();
    site_builds.sort_by_key(|build| build.site.id.0);
    let sites = site_builds
        .iter()
        .map(|build| build.site)
        .collect::<Vec<_>>();
    let biomes = site_builds
        .iter()
        .map(|build| build.biome)
        .collect::<Vec<_>>();

    let site_map = sites
        .iter()
        .map(|site| (site.id, *site))
        .collect::<HashMap<_, _>>();
    let corner_sites = corner_site_neighbors(patch);
    let mut corners = patch
        .corners
        .par_iter()
        .map(|corner| macro_corner(corner, &corner_sites, &site_map, config))
        .collect::<Vec<_>>();
    corners.sort_by_key(|corner| corner.id.0);
    let corner_map = corners
        .iter()
        .map(|corner| (corner.id, *corner))
        .collect::<HashMap<_, _>>();

    let mut edges = patch
        .edges
        .par_iter()
        .map(|edge| {
            let a = site_map.get(&edge.sites[0]).copied();
            let b = site_map.get(&edge.sites[1]).copied();
            MacroEdge {
                id: edge.id,
                sites: edge.sites,
                corners: edge.corners,
                guide: macro_edge_guide_with_corners(a, b, edge.corners, &corner_map, config),
                lake_class: macro_edge_lake_class(a, b, edge.corners, &corner_map),
            }
        })
        .collect::<Vec<_>>();
    edges.sort_by_key(|edge| edge.id.0);

    GraphMacroMap {
        sites,
        corners,
        edges,
        biomes,
    }
}

#[derive(Debug, Clone, Copy)]
struct MacroSiteBuild {
    site: MacroSite,
    biome: GraphBiomeCell,
}

#[derive(Debug, Clone, Copy)]
struct SiteContext {
    base_fields: GraphBaseFields,
    ruggedness: f32,
    feature_hash: u64,
    is_land_owned: bool,
    is_island_owned: bool,
    is_tiny_local_minima_lake: bool,
    is_inland_water: bool,
    inland_water_surface: Option<MacroSurfaceKind>,
    component_id: u64,
    graph_distance_to_coast: u32,
    spacing_blocks: f32,
}

fn validate_macro_map_config(config: MacroMapConfig) {
    assert!(
        config.coast_width_blocks.is_finite() && config.coast_width_blocks > 0.0,
        "coast_width_blocks must be positive and finite"
    );
    assert!(config.sea_level.is_finite(), "sea_level must be finite");
    assert!(
        config.ridge_candidate_threshold.is_finite(),
        "ridge_candidate_threshold must be finite"
    );
    assert!(
        config.river_candidate_threshold.is_finite(),
        "river_candidate_threshold must be finite"
    );
    assert!(config.land_bias.is_finite(), "land_bias must be finite");
}

fn resolve_site_context(patch: &VoronoiGraphPatch, config: MacroMapConfig) -> Vec<SiteContext> {
    let site_indices = patch
        .sites
        .iter()
        .enumerate()
        .map(|(index, site)| (site.id, index))
        .collect::<HashMap<_, _>>();
    let adjacency = site_adjacency(patch, &site_indices);
    let land_mask = patch
        .sites
        .iter()
        .map(|site| is_land_base(site.base_fields, config))
        .collect::<Vec<_>>();
    let components = connected_components(&patch.sites, &adjacency, &land_mask);
    let component_sizes = component_sizes(&components);
    let ocean_components =
        explicit_ocean_components(&patch.sites, &components, &land_mask, &component_sizes);
    let ocean_mask = components
        .iter()
        .enumerate()
        .map(|(index, component)| {
            !land_mask[index] && ocean_components.get(component).copied().unwrap_or(false)
        })
        .collect::<Vec<_>>();
    let coast_distances = graph_distances_to_ocean_coast(&adjacency, &ocean_mask);
    let tiny_local_minima_lakes = tiny_local_minima_lake_mask(
        &patch.sites,
        &adjacency,
        &land_mask,
        &coast_distances,
        config,
    );
    let largest_land_component = components
        .iter()
        .enumerate()
        .filter(|(index, _)| land_mask[*index])
        .max_by_key(|(_, component)| component_sizes.get(component).copied().unwrap_or(0))
        .map(|(_, component)| *component);
    let island_size_limit = largest_land_component
        .and_then(|component| component_sizes.get(&component).copied())
        .map(|size| (size / 3).max(6))
        .unwrap_or(usize::MAX);
    let spacing_blocks = DEFAULT_MACRO_GRAPH_DISTANCE_STEP_BLOCKS;

    patch
        .sites
        .iter()
        .enumerate()
        .map(|(index, site)| {
            let component = components[index];
            let component_size = component_sizes.get(&component).copied().unwrap_or(0);
            let is_inland_water =
                !land_mask[index] && !ocean_components.get(&component).copied().unwrap_or(false);
            let inland_water_surface = is_inland_water
                .then(|| inland_water_surface_kind(component, component_size, site.base_fields));
            SiteContext {
                base_fields: site.base_fields,
                ruggedness: site.ruggedness,
                feature_hash: splitmix64(site.id.0 ^ SMALL_STREAM_POCKET_LAKE_NAMESPACE),
                is_land_owned: land_mask[index],
                is_island_owned: land_mask[index]
                    && Some(component) != largest_land_component
                    && component_size <= island_size_limit,
                is_tiny_local_minima_lake: tiny_local_minima_lakes[index],
                is_inland_water,
                inland_water_surface,
                component_id: component,
                graph_distance_to_coast: coast_distances[index],
                spacing_blocks,
            }
        })
        .collect()
}

fn inland_water_surface_kind(
    component: u64,
    component_size: usize,
    fields: GraphBaseFields,
) -> MacroSurfaceKind {
    let deep_basin = fields.elevation_seed < -0.42 || fields.hydration >= 0.62;
    let rare_large_lake = component_size <= 30
        && splitmix64(component ^ 0x49f1_69b4_8f7d_21ab) % 100 < 13
        && deep_basin;

    if component_size <= 10 || rare_large_lake {
        MacroSurfaceKind::LakeCandidate
    } else if component_size <= 30 && fields.hydration >= 0.50 {
        MacroSurfaceKind::WetlandCandidate
    } else if fields.hydration >= 0.74 && fields.elevation_seed < -0.18 {
        MacroSurfaceKind::WetlandCandidate
    } else {
        MacroSurfaceKind::DryBasin
    }
}

fn tiny_local_minima_lake_mask(
    sites: &[super::graph::VoronoiSite],
    adjacency: &[Vec<usize>],
    land_mask: &[bool],
    coast_distances: &[u32],
    config: MacroMapConfig,
) -> Vec<bool> {
    let local_heights = sites
        .iter()
        .map(|site| tiny_local_minima_height(site.base_fields))
        .collect::<Vec<_>>();
    let candidates = sites
        .iter()
        .enumerate()
        .map(|(index, site)| {
            if !land_mask[index] || coast_distances[index] <= 1 {
                return false;
            }

            let height = local_heights[index];
            let has_lower_land_neighbor = adjacency[index]
                .iter()
                .any(|&neighbor| land_mask[neighbor] && local_heights[neighbor] < height - 0.006);

            !has_lower_land_neighbor && tiny_local_minima_site_score(site.base_fields) >= 0.58
        })
        .collect::<Vec<_>>();
    let mut promoted = vec![false; sites.len()];
    let mut visited = vec![false; sites.len()];

    for start in 0..sites.len() {
        if visited[start] || !candidates[start] {
            continue;
        }

        let mut stack = vec![start];
        let mut members = Vec::new();
        visited[start] = true;

        while let Some(index) = stack.pop() {
            members.push(index);
            for &neighbor in &adjacency[index] {
                if visited[neighbor] || !candidates[neighbor] {
                    continue;
                }
                visited[neighbor] = true;
                stack.push(neighbor);
            }
        }

        if members.len() > DEFAULT_TINY_LOCAL_MINIMA_LAKE_MAX_CELLS {
            continue;
        }

        let rim_height = members
            .iter()
            .flat_map(|&member| adjacency[member].iter().copied())
            .filter(|neighbor| land_mask[*neighbor] && !members.contains(neighbor))
            .map(|neighbor| local_heights[neighbor])
            .min_by(|a, b| a.total_cmp(b))
            .unwrap_or(f32::INFINITY);
        let floor_height = members
            .iter()
            .map(|&member| local_heights[member])
            .max_by(|a, b| a.total_cmp(b))
            .unwrap_or(f32::INFINITY);
        let rim_relief = (rim_height - floor_height).max(0.0);
        if !rim_relief.is_finite() || rim_relief < 0.006 {
            continue;
        }

        let score = members
            .iter()
            .map(|&member| tiny_local_minima_site_score(sites[member].base_fields))
            .sum::<f32>()
            / members.len().max(1) as f32;
        let min_site_id = members
            .iter()
            .map(|&member| sites[member].id.0)
            .min()
            .unwrap_or(0);
        let component_hash = splitmix64(
            min_site_id
                ^ config.seed
                ^ ((config.generator_version as u64) << 32)
                ^ TINY_LOCAL_MINIMA_LAKE_NAMESPACE,
        );

        if tiny_local_minima_lake_roll(members.len(), score, component_hash) {
            for member in members {
                promoted[member] = true;
            }
        }
    }

    promoted
}

fn tiny_local_minima_height(fields: GraphBaseFields) -> f32 {
    fields.elevation_seed * 0.72 + fields.continentality.max(0.0) * 0.18 - fields.hydration * 0.10
}

fn tiny_local_minima_site_score(fields: GraphBaseFields) -> f32 {
    let low_elevation = (1.0 - ((fields.elevation_seed + 1.0) * 0.5)).clamp(0.0, 1.0);
    (low_elevation * 0.62 + fields.hydration * 0.38).clamp(0.0, 1.0)
}

fn tiny_local_minima_lake_roll(component_size: usize, score: f32, component_hash: u64) -> bool {
    if !(1..=DEFAULT_TINY_LOCAL_MINIMA_LAKE_MAX_CELLS).contains(&component_size) || score < 0.58 {
        return false;
    }

    let score_bonus = if score >= 0.82 {
        300
    } else if score >= 0.74 {
        150
    } else {
        0
    };
    let chance_per_10k = (DEFAULT_TINY_LOCAL_MINIMA_LAKE_CHANCE_PER_10K + score_bonus)
        .min(DEFAULT_TINY_LOCAL_MINIMA_LAKE_MAX_CHANCE_PER_10K);

    ((component_hash % 10_000) as u32) < chance_per_10k
}

fn component_sizes(components: &[u64]) -> HashMap<u64, usize> {
    let mut sizes = HashMap::new();
    for &component in components {
        *sizes.entry(component).or_insert(0) += 1;
    }
    sizes
}

fn explicit_ocean_components(
    sites: &[super::graph::VoronoiSite],
    components: &[u64],
    land_mask: &[bool],
    component_sizes: &HashMap<u64, usize>,
) -> HashMap<u64, bool> {
    let largest_water_component = components
        .iter()
        .enumerate()
        .filter(|(index, _)| !land_mask[*index])
        .max_by_key(|(_, component)| component_sizes.get(component).copied().unwrap_or(0))
        .map(|(_, component)| *component);
    let largest_water_size = largest_water_component
        .and_then(|component| component_sizes.get(&component).copied())
        .unwrap_or(0);
    let ocean_size_floor = ((largest_water_size as f32) * 0.18).round().max(64.0) as usize;

    let mut continentality_sum = HashMap::<u64, f32>::new();
    let mut counts = HashMap::<u64, usize>::new();
    for ((site, &component), &is_land) in sites.iter().zip(components).zip(land_mask) {
        if is_land {
            continue;
        }
        *continentality_sum.entry(component).or_default() += site.base_fields.continentality;
        *counts.entry(component).or_default() += 1;
    }

    let mut ocean_components = HashMap::new();
    for (&component, &count) in &counts {
        let average_continentality =
            continentality_sum.get(&component).copied().unwrap_or(0.0) / count.max(1) as f32;
        let is_primary_ocean = Some(component) == largest_water_component;
        let is_explicit_secondary_ocean =
            count >= ocean_size_floor && average_continentality <= -0.18;
        ocean_components.insert(component, is_primary_ocean || is_explicit_secondary_ocean);
    }

    ocean_components
}

fn is_land_base(fields: GraphBaseFields, config: MacroMapConfig) -> bool {
    clamp_signed(fields.continentality + config.land_bias - config.sea_level) >= 0.0
}

fn site_adjacency(
    patch: &VoronoiGraphPatch,
    site_indices: &HashMap<VoronoiSiteId, usize>,
) -> Vec<Vec<usize>> {
    let mut adjacency = vec![Vec::new(); patch.sites.len()];

    for edge in &patch.edges {
        let Some(&a) = site_indices.get(&edge.sites[0]) else {
            continue;
        };
        let Some(&b) = site_indices.get(&edge.sites[1]) else {
            continue;
        };
        adjacency[a].push(b);
        adjacency[b].push(a);
    }

    adjacency.par_iter_mut().for_each(|neighbors| {
        neighbors.sort_unstable();
        neighbors.dedup();
    });

    adjacency
}

fn connected_components(
    sites: &[super::graph::VoronoiSite],
    adjacency: &[Vec<usize>],
    land_mask: &[bool],
) -> Vec<u64> {
    let mut components = vec![0; sites.len()];
    let mut visited = vec![false; sites.len()];

    for start in 0..sites.len() {
        if visited[start] {
            continue;
        }

        let is_land = land_mask[start];
        let namespace = if is_land {
            LAND_COMPONENT_NAMESPACE
        } else {
            OCEAN_COMPONENT_NAMESPACE
        };
        let mut stack = vec![start];
        let mut members = Vec::new();
        let mut min_site_id = sites[start].id.0;
        visited[start] = true;

        while let Some(index) = stack.pop() {
            members.push(index);
            min_site_id = min_site_id.min(sites[index].id.0);

            for &neighbor in &adjacency[index] {
                if visited[neighbor] || land_mask[neighbor] != is_land {
                    continue;
                }
                visited[neighbor] = true;
                stack.push(neighbor);
            }
        }

        let component_id = splitmix64(min_site_id ^ namespace);
        for index in members {
            components[index] = component_id;
        }
    }

    components
}

fn graph_distances_to_ocean_coast(adjacency: &[Vec<usize>], ocean_mask: &[bool]) -> Vec<u32> {
    let mut distances = vec![u32::MAX; adjacency.len()];
    let mut queue = VecDeque::new();

    for (index, neighbors) in adjacency.iter().enumerate() {
        if neighbors
            .iter()
            .any(|&neighbor| ocean_mask[neighbor] != ocean_mask[index])
        {
            distances[index] = 0;
            queue.push_back(index);
        }
    }

    while let Some(index) = queue.pop_front() {
        let next_distance = distances[index].saturating_add(1);
        for &neighbor in &adjacency[index] {
            if ocean_mask[neighbor] != ocean_mask[index] || distances[neighbor] <= next_distance {
                continue;
            }
            distances[neighbor] = next_distance;
            queue.push_back(neighbor);
        }
    }

    distances
}

fn macro_site_from_context(
    id: VoronoiSiteId,
    owner_region: GraphRegionCoord,
    position: WorldPlanePoint,
    context: SiteContext,
    config: MacroMapConfig,
) -> MacroSiteBuild {
    let sample = macro_field_sample_from_context(context, config);
    let macro_land_side = sample.surface_kind.is_land_owned();
    let site = MacroSite {
        id,
        owner_region,
        position,
        surface_kind: sample.surface_kind,
        continent: macro_land_side.then_some(MacroContinentId(context.component_id)),
        ocean_basin: (!macro_land_side).then_some(MacroOceanBasinId(context.component_id)),
        signed_macro_elevation: sample.signed_macro_elevation,
        continentality: sample.continentality,
        coastness: sample.coastness,
        distance_to_coast_blocks: sample.distance_to_coast_blocks,
        distance_to_continent_core_blocks: if macro_land_side {
            sample.distance_to_coast_blocks
        } else {
            0.0
        },
        distance_to_ocean_basin_blocks: if macro_land_side {
            sample.distance_to_coast_blocks
        } else {
            sample.distance_to_coast_blocks
        },
        mountainness: sample.mountainness,
        ridgeness: sample.ridgeness,
        basinness: sample.basinness,
    };
    MacroSiteBuild {
        site,
        biome: GraphBiomeCell {
            site: id,
            context: sample.biome_context,
            biome: sample.biome,
        },
    }
}

#[derive(Debug, Clone, Copy)]
struct MacroFieldSample {
    surface_kind: MacroSurfaceKind,
    biome_context: GraphBiomeContext,
    biome: super::biome::GraphBiomeKind,
    signed_macro_elevation: f32,
    continentality: f32,
    coastness: f32,
    distance_to_coast_blocks: f32,
    mountainness: f32,
    ridgeness: f32,
    basinness: f32,
}

fn macro_field_sample_from_context(
    context: SiteContext,
    config: MacroMapConfig,
) -> MacroFieldSample {
    let fields = context.base_fields;
    let dry_basin = matches!(
        context.inland_water_surface,
        Some(MacroSurfaceKind::DryBasin)
    );
    let effective_land_owned = context.is_land_owned || dry_basin;
    let continentality = clamp_signed(fields.continentality + config.land_bias - config.sea_level);
    let raw_distance_to_coast_blocks = if context.graph_distance_to_coast == u32::MAX {
        config.coast_width_blocks * 8.0
    } else {
        context.graph_distance_to_coast as f32 * context.spacing_blocks
    };
    let distance_to_coast_blocks =
        raw_distance_to_coast_blocks.min(config.coast_width_blocks * 4.0);
    let coastness = (1.0 - distance_to_coast_blocks / config.coast_width_blocks).clamp(0.0, 1.0);
    let elevation_seed = fields.elevation_seed;
    let highland_signal =
        elevation_seed * 0.50 + continentality.max(0.0) * 0.24 + context.ruggedness * 0.12;
    let mountainness = if effective_land_owned {
        smoothstep(0.18, 0.78, highland_signal)
    } else {
        0.0
    };
    let ridgeness = if effective_land_owned {
        smoothstep(
            0.34,
            0.82,
            highland_signal * 0.56 + context.ruggedness * 0.28 + mountainness * 0.16,
        )
    } else {
        0.0
    };
    let basinness = if effective_land_owned {
        clamp_unit(
            (1.0 - mountainness) * 0.34
                + fields.hydration * 0.25
                + (1.0 - elevation_seed.max(0.0)) * 0.19,
        )
    } else {
        let inlandness =
            (distance_to_coast_blocks / (config.coast_width_blocks * 4.0)).clamp(0.0, 1.0);
        clamp_unit((-continentality).max(0.0) * 0.70 + inlandness * 0.30)
    };
    let raw_signed_macro_elevation = signed_macro_elevation_from_continentality_and_ruggedness(
        continentality,
        elevation_seed,
        context.ruggedness,
    );
    let signed_macro_elevation = raw_signed_macro_elevation.clamp(-1.0, 1.5);

    let surface_kind = surface_kind(
        fields,
        context.is_land_owned,
        context.is_island_owned,
        context.is_tiny_local_minima_lake,
        context.is_inland_water,
        context.inland_water_surface,
        coastness,
        basinness,
        signed_macro_elevation.max(0.0),
        context.feature_hash,
    );
    let biome_context = graph_biome_context(
        fields,
        surface_kind,
        signed_macro_elevation,
        continentality,
        coastness,
        mountainness,
        ridgeness,
    );
    let biome = classify_graph_biome(biome_context);

    MacroFieldSample {
        surface_kind,
        biome_context,
        biome,
        signed_macro_elevation,
        continentality,
        coastness,
        distance_to_coast_blocks,
        mountainness,
        ridgeness,
        basinness,
    }
}

fn graph_biome_context(
    fields: GraphBaseFields,
    surface_kind: MacroSurfaceKind,
    signed_macro_elevation: f32,
    continentality: f32,
    coastness: f32,
    mountainness: f32,
    ruggedness: f32,
) -> GraphBiomeContext {
    GraphBiomeContext {
        temperature: fields.temperature,
        hydration: fields.hydration,
        elevation: signed_macro_elevation,
        continentality,
        coastness,
        mountainness,
        ruggedness,
        water_role: graph_biome_water_role(surface_kind, signed_macro_elevation, coastness),
    }
    .clamped()
}

fn graph_biome_water_role(
    surface_kind: MacroSurfaceKind,
    signed_macro_elevation: f32,
    coastness: f32,
) -> GraphBiomeWaterRole {
    match surface_kind {
        MacroSurfaceKind::OceanBasin | MacroSurfaceKind::CoastOcean => {
            if signed_macro_elevation <= -0.32 && coastness < 0.25 {
                GraphBiomeWaterRole::DeepOcean
            } else {
                GraphBiomeWaterRole::ShallowOcean
            }
        }
        MacroSurfaceKind::CoastLand | MacroSurfaceKind::CoastIsland => GraphBiomeWaterRole::Coast,
        MacroSurfaceKind::LakeCandidate => GraphBiomeWaterRole::Lake,
        MacroSurfaceKind::WetlandCandidate => GraphBiomeWaterRole::Wetland,
        MacroSurfaceKind::DryBasin => GraphBiomeWaterRole::DryBasin,
        MacroSurfaceKind::Continent | MacroSurfaceKind::Island => GraphBiomeWaterRole::Land,
    }
}

fn signed_macro_elevation_from_continentality_and_ruggedness(
    continentality: f32,
    elevation_seed: f32,
    ruggedness: f32,
) -> f32 {
    let sea_level_signal = clamp_signed(continentality);
    let ruggedness = clamp_unit(ruggedness);
    let magnitude = sea_level_signal.abs();
    let shaped_signal = if sea_level_signal >= 0.0 {
        magnitude.powf(1.24)
    } else {
        -smoothstep(0.0, 1.0, magnitude)
    };
    let relief_gate = if sea_level_signal >= 0.0 {
        magnitude.powf(1.12)
    } else {
        smoothstep(0.0, 1.0, magnitude)
    };
    let local_relief = clamp_signed(elevation_seed) * (0.10 + ruggedness * 0.55) * relief_gate;
    let base_scale = 0.78 + ruggedness * 0.22;

    shaped_signal * base_scale + local_relief
}

fn corner_site_neighbors(
    patch: &VoronoiGraphPatch,
) -> HashMap<VoronoiCornerId, Vec<VoronoiSiteId>> {
    let mut neighbors = HashMap::<VoronoiCornerId, Vec<VoronoiSiteId>>::new();

    for edge in &patch.edges {
        for corner in edge.corners {
            let entry = neighbors.entry(corner).or_default();
            entry.extend(edge.sites);
        }
    }

    neighbors.par_iter_mut().for_each(|(_, sites)| {
        sites.sort_by_key(|site| site.0);
        sites.dedup();
    });

    neighbors
}

fn macro_corner(
    corner: &super::graph::VoronoiCorner,
    corner_sites: &HashMap<VoronoiCornerId, Vec<VoronoiSiteId>>,
    site_map: &HashMap<VoronoiSiteId, MacroSite>,
    config: MacroMapConfig,
) -> MacroCorner {
    let adjacent_sites = corner_sites
        .get(&corner.id)
        .into_iter()
        .flatten()
        .filter_map(|site_id| site_map.get(site_id).copied())
        .collect::<Vec<_>>();
    let is_land_owned = if adjacent_sites.is_empty() {
        is_land_base(corner.base_fields, config)
    } else {
        let land_count = adjacent_sites
            .iter()
            .filter(|site| site.surface_kind.is_land_owned())
            .count();
        land_count * 2 >= adjacent_sites.len()
    };
    let lake_neighbor_count = adjacent_sites
        .iter()
        .filter(|site| {
            matches!(
                site.surface_kind,
                MacroSurfaceKind::LakeCandidate | MacroSurfaceKind::WetlandCandidate
            )
        })
        .count();
    let is_inland_water =
        !adjacent_sites.is_empty() && lake_neighbor_count * 2 >= adjacent_sites.len();
    let dry_basin_neighbor_count = adjacent_sites
        .iter()
        .filter(|site| matches!(site.surface_kind, MacroSurfaceKind::DryBasin))
        .count();
    let inland_water_surface = if is_inland_water {
        Some(MacroSurfaceKind::LakeCandidate)
    } else if !adjacent_sites.is_empty() && dry_basin_neighbor_count * 2 >= adjacent_sites.len() {
        Some(MacroSurfaceKind::DryBasin)
    } else {
        None
    };
    let base_is_land_owned = is_land_owned && !is_inland_water;
    let component_site = adjacent_sites
        .iter()
        .copied()
        .filter(|site| {
            if is_inland_water {
                matches!(
                    site.surface_kind,
                    MacroSurfaceKind::LakeCandidate | MacroSurfaceKind::WetlandCandidate
                )
            } else {
                site.surface_kind.is_land_owned() == is_land_owned
            }
        })
        .min_by_key(|site| site.id.0);
    let is_island_owned = component_site.is_some_and(|site| {
        matches!(
            site.surface_kind,
            MacroSurfaceKind::Island | MacroSurfaceKind::CoastIsland
        )
    });
    let coast_distance = adjacent_sites
        .iter()
        .map(|site| site.distance_to_coast_blocks)
        .min_by(|a, b| a.total_cmp(b))
        .unwrap_or(config.coast_width_blocks * 8.0);
    let context = SiteContext {
        base_fields: corner.base_fields,
        ruggedness: adjacent_sites
            .iter()
            .map(|site| site.ridgeness)
            .sum::<f32>()
            / adjacent_sites.len().max(1) as f32,
        feature_hash: splitmix64(corner.id.0 ^ SMALL_STREAM_POCKET_LAKE_NAMESPACE),
        is_land_owned: base_is_land_owned,
        is_island_owned,
        is_tiny_local_minima_lake: false,
        is_inland_water,
        inland_water_surface,
        component_id: component_site
            .and_then(|site| {
                site.continent
                    .map(|id| id.0)
                    .or(site.ocean_basin.map(|id| id.0))
            })
            .unwrap_or_else(|| splitmix64(corner.id.0)),
        graph_distance_to_coast: (coast_distance / DEFAULT_MACRO_GRAPH_DISTANCE_STEP_BLOCKS)
            .round()
            .max(0.0) as u32,
        spacing_blocks: DEFAULT_MACRO_GRAPH_DISTANCE_STEP_BLOCKS,
    };
    let sample = macro_field_sample_from_context(context, config);
    let macro_land_side = sample.surface_kind.is_land_owned();

    MacroCorner {
        id: corner.id,
        position: corner.position,
        surface_kind: sample.surface_kind,
        continent: macro_land_side.then_some(MacroContinentId(context.component_id)),
        ocean_basin: (!macro_land_side).then_some(MacroOceanBasinId(context.component_id)),
        signed_macro_elevation: sample.signed_macro_elevation,
        continentality: sample.continentality,
        coastness: sample.coastness,
        distance_to_coast_blocks: sample.distance_to_coast_blocks,
        mountainness: sample.mountainness,
        ridgeness: sample.ridgeness,
        basinness: sample.basinness,
    }
}

fn surface_kind(
    fields: GraphBaseFields,
    is_land_owned: bool,
    is_island_owned: bool,
    is_tiny_local_minima_lake: bool,
    is_inland_water: bool,
    inland_water_surface: Option<MacroSurfaceKind>,
    coastness: f32,
    basinness: f32,
    signed_macro_elevation: f32,
    feature_hash: u64,
) -> MacroSurfaceKind {
    if is_inland_water {
        inland_water_surface.unwrap_or(MacroSurfaceKind::LakeCandidate)
    } else if matches!(inland_water_surface, Some(MacroSurfaceKind::DryBasin)) {
        MacroSurfaceKind::DryBasin
    } else if is_land_owned {
        if coastness >= 0.55 {
            if is_island_owned {
                MacroSurfaceKind::CoastIsland
            } else {
                MacroSurfaceKind::CoastLand
            }
        } else if is_island_owned {
            MacroSurfaceKind::Island
        } else if is_tiny_local_minima_lake {
            MacroSurfaceKind::LakeCandidate
        } else if small_stream_pocket_lake(
            fields,
            coastness,
            basinness,
            signed_macro_elevation,
            feature_hash,
        ) {
            MacroSurfaceKind::LakeCandidate
        } else if basinness >= 0.88 && signed_macro_elevation <= 0.08 {
            MacroSurfaceKind::LakeCandidate
        } else if basinness >= 0.78 && signed_macro_elevation <= 0.16 {
            MacroSurfaceKind::WetlandCandidate
        } else {
            MacroSurfaceKind::Continent
        }
    } else if coastness >= 0.55 {
        MacroSurfaceKind::CoastOcean
    } else {
        MacroSurfaceKind::OceanBasin
    }
}

fn small_stream_pocket_lake(
    fields: GraphBaseFields,
    coastness: f32,
    basinness: f32,
    signed_macro_elevation: f32,
    feature_hash: u64,
) -> bool {
    if coastness >= 0.42 || signed_macro_elevation > 0.22 {
        return false;
    }

    let low_elevation = (1.0 - ((fields.elevation_seed + 1.0) * 0.5)).clamp(0.0, 1.0);
    let local_minima_score = basinness * 0.50 + fields.hydration * 0.28 + low_elevation * 0.22;
    let deterministic_roll = (feature_hash % 10_000) as u32;
    let chance_per_10k = if local_minima_score >= 0.90 {
        900
    } else if local_minima_score >= 0.84 {
        520
    } else if local_minima_score >= 0.79 {
        260
    } else {
        0
    };

    deterministic_roll < chance_per_10k
}

#[cfg(test)]
fn macro_edge_guide(
    a: Option<MacroSite>,
    b: Option<MacroSite>,
    config: MacroMapConfig,
) -> MacroEdgeGuide {
    macro_edge_guide_with_corner_kinds(a, b, None, config)
}

fn macro_edge_guide_with_corners(
    a: Option<MacroSite>,
    b: Option<MacroSite>,
    corners: [VoronoiCornerId; 2],
    corner_map: &HashMap<VoronoiCornerId, MacroCorner>,
    config: MacroMapConfig,
) -> MacroEdgeGuide {
    let corner_kinds = [
        corner_map
            .get(&corners[0])
            .map(|corner| corner.surface_kind),
        corner_map
            .get(&corners[1])
            .map(|corner| corner.surface_kind),
    ];
    macro_edge_guide_with_corner_kinds(a, b, Some(corner_kinds), config)
}

fn macro_edge_guide_with_corner_kinds(
    a: Option<MacroSite>,
    b: Option<MacroSite>,
    corner_kinds: Option<[Option<MacroSurfaceKind>; 2]>,
    config: MacroMapConfig,
) -> MacroEdgeGuide {
    let Some(a) = a else {
        return empty_edge_guide();
    };
    let Some(b) = b else {
        return empty_edge_guide();
    };

    let a_land = a.surface_kind.is_land_owned();
    let b_land = b.surface_kind.is_land_owned();
    let is_coast = macro_edge_is_explicit_ocean_boundary(a, b, corner_kinds);
    let coastness = if is_coast {
        1.0
    } else {
        ((a.coastness + b.coastness) * 0.5).clamp(0.0, 1.0)
    };
    let average_elevation = (a.signed_macro_elevation + b.signed_macro_elevation) * 0.5;
    let signed_elevation_gradient = b.signed_macro_elevation - a.signed_macro_elevation;
    let elevation_slope = signed_elevation_gradient.abs();
    let both_land = a_land && b_land;
    let same_land_component = both_land && a.continent.is_some() && a.continent == b.continent;
    let average_coastness = (a.coastness + b.coastness) * 0.5;
    let minimum_inland_distance = a.distance_to_coast_blocks.min(b.distance_to_coast_blocks);
    let inlandness = (minimum_inland_distance / (config.coast_width_blocks * 2.5)).clamp(0.0, 1.0);
    let average_mountainness = (a.mountainness + b.mountainness) * 0.5;
    let average_ruggedness = (a.ridgeness + b.ridgeness) * 0.5;
    let mountainness = if same_land_component {
        (average_mountainness * 0.48
            + average_ruggedness * 0.24
            + smoothstep(0.12, 0.58, average_elevation) * 0.14
            + inlandness * 0.14)
            .clamp(0.0, 1.0)
    } else {
        0.0
    };
    let drainage_divide_potential = if same_land_component {
        (mountainness * 0.34
            + average_ruggedness * 0.24
            + (1.0 - ((a.basinness + b.basinness) * 0.5)) * 0.18
            + inlandness * 0.14
            + smoothstep(0.015, 0.16, elevation_slope) * 0.10)
            .clamp(0.0, 1.0)
    } else {
        0.0
    };
    let gradient_score = smoothstep(0.012, 0.14, elevation_slope);
    let ridgeness = if same_land_component {
        (average_ruggedness * 0.30
            + mountainness * 0.20
            + drainage_divide_potential * 0.24
            + gradient_score * 0.16
            + inlandness * 0.10)
            .clamp(0.0, 1.0)
    } else {
        0.0
    };
    let has_ridge_context = same_land_component
        && mountainness >= 0.16
        && average_elevation > 0.05
        && average_coastness < 0.80
        && inlandness >= 0.12;
    let is_ridge_candidate = has_ridge_context
        && ridgeness >= config.ridge_candidate_threshold
        && gradient_score >= 0.01
        && drainage_divide_potential >= 0.22;
    let is_fault_candidate = same_land_component
        && elevation_slope > 0.06
        && mountainness >= 0.20
        && average_coastness < 0.78
        && inlandness >= 0.14;

    MacroEdgeGuide {
        is_coast,
        is_ridge_candidate,
        is_river_candidate: false,
        is_fault_candidate,
        coastness,
        mountainness,
        ridgeness,
        signed_elevation_gradient,
        drainage_divide_potential,
        river_potential: 0.0,
    }
}

fn macro_edge_is_explicit_ocean_boundary(
    a: MacroSite,
    b: MacroSite,
    corner_kinds: Option<[Option<MacroSurfaceKind>; 2]>,
) -> bool {
    if is_ocean_to_non_ocean_coast_pair(a.surface_kind, b.surface_kind) {
        return true;
    }

    let Some([Some(first), Some(second)]) = corner_kinds else {
        return false;
    };
    is_ocean_to_non_ocean_coast_pair(first, second)
}

fn is_ocean_to_non_ocean_coast_pair(a: MacroSurfaceKind, b: MacroSurfaceKind) -> bool {
    if is_lake_surface(a) || is_lake_surface(b) {
        return false;
    }
    a.is_ocean_owned() != b.is_ocean_owned()
}

fn macro_edge_lake_class(
    a: Option<MacroSite>,
    b: Option<MacroSite>,
    corners: [VoronoiCornerId; 2],
    corner_map: &HashMap<VoronoiCornerId, MacroCorner>,
) -> MacroLakeEdgeClass {
    let lake_sites = [a, b]
        .into_iter()
        .flatten()
        .filter(|site| is_lake_surface(site.surface_kind))
        .count();
    let lake_corners = corners
        .into_iter()
        .filter(|corner| {
            corner_map
                .get(corner)
                .is_some_and(|corner| is_lake_surface(corner.surface_kind))
        })
        .count();

    match (lake_sites, lake_corners) {
        (2, _) | (_, 2) => MacroLakeEdgeClass::LakeInternal,
        (1, _) => MacroLakeEdgeClass::LakeBoundary,
        (0, 1) => MacroLakeEdgeClass::LakeAdjacentLand,
        _ => MacroLakeEdgeClass::NonLake,
    }
}

fn is_lake_surface(surface: MacroSurfaceKind) -> bool {
    matches!(
        surface,
        MacroSurfaceKind::LakeCandidate | MacroSurfaceKind::WetlandCandidate
    )
}

fn empty_edge_guide() -> MacroEdgeGuide {
    MacroEdgeGuide {
        is_coast: false,
        is_ridge_candidate: false,
        is_river_candidate: false,
        is_fault_candidate: false,
        coastness: 0.0,
        mountainness: 0.0,
        ridgeness: 0.0,
        signed_elevation_gradient: 0.0,
        drainage_divide_potential: 0.0,
        river_potential: 0.0,
    }
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    if (edge1 - edge0).abs() <= f32::EPSILON {
        return (value >= edge1) as u8 as f32;
    }

    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn clamp_unit(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

fn clamp_signed(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::super::biome::GraphBiomeKind;
    use super::*;
    use crate::world::generation::graph::{
        generate_voronoi_graph_patch, VoronoiGraphConfig, VoronoiGraphPatchRequest,
        DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS,
    };
    use std::collections::{HashMap, HashSet};

    #[test]
    fn macro_map_generation_is_deterministic() {
        let patch = generate_voronoi_graph_patch(test_request(42, 0, 0));
        let config = test_macro_config(42);

        let first = generate_macro_map(&patch, config);
        let second = generate_macro_map(&patch, config);

        assert_eq!(first, second);
    }

    #[test]
    fn macro_map_generation_changes_with_seed() {
        let first_patch = generate_voronoi_graph_patch(test_request(42, 0, 0));
        let second_patch = generate_voronoi_graph_patch(test_request(43, 0, 0));

        let first = generate_macro_map(&first_patch, test_macro_config(42));
        let second = generate_macro_map(&second_patch, test_macro_config(43));

        assert_ne!(first.sites, second.sites);
    }

    #[test]
    fn macro_map_contains_land_and_ocean_sites() {
        let map = (7..96)
            .find_map(|seed| {
                let patch = generate_voronoi_graph_patch(test_request(seed, 0, 0));
                let map = generate_macro_map(&patch, test_macro_config(seed));
                let has_land = map
                    .sites
                    .iter()
                    .any(|site| site.surface_kind.is_land_owned());
                let has_ocean = map
                    .sites
                    .iter()
                    .any(|site| site.surface_kind.is_ocean_owned());

                (has_land && has_ocean).then_some(map)
            })
            .expect("expected at least one deterministic seed with both land and ocean");

        assert!(map
            .sites
            .iter()
            .any(|site| site.signed_macro_elevation > 0.0));
        assert!(map
            .sites
            .iter()
            .any(|site| site.signed_macro_elevation < 0.0));
    }

    #[test]
    fn macro_ownership_follows_graph_continentality() {
        let patch = generate_voronoi_graph_patch(test_request(31, 0, 0));
        let config = test_macro_config(31);
        let map = generate_macro_map(&patch, config);
        let macro_sites = macro_sites_by_id(&map);

        for site in &patch.sites {
            let macro_site = macro_sites[&site.id];
            let graph_land = site.base_fields.continentality + config.land_bias >= 0.0;
            if graph_land {
                assert!(macro_site.surface_kind.is_land_owned());
            } else {
                assert!(
                    macro_site.surface_kind.is_ocean_owned()
                        || matches!(
                            macro_site.surface_kind,
                            MacroSurfaceKind::DryBasin
                                | MacroSurfaceKind::LakeCandidate
                                | MacroSurfaceKind::WetlandCandidate
                        )
                );
            }
            assert_close(
                macro_site.continentality,
                clamp_signed(site.base_fields.continentality + config.land_bias),
            );
        }
    }

    #[test]
    fn inland_water_components_resolve_as_lake_candidates() {
        let patch = generate_voronoi_graph_patch(preview_like_request(42, 0, 0));
        let map = generate_macro_map(&patch, MacroMapConfig::new(42, 11));
        let default_window_lakes = map
            .sites
            .iter()
            .filter(|site| site.position.x >= -16_384.0)
            .filter(|site| site.position.x <= 16_384.0)
            .filter(|site| site.position.z >= -9_216.0)
            .filter(|site| site.position.z <= 9_216.0)
            .filter(|site| matches!(site.surface_kind, MacroSurfaceKind::LakeCandidate))
            .filter_map(|site| site.continent)
            .collect::<HashSet<_>>();

        assert!(
            !default_window_lakes.is_empty(),
            "seed 42 default macro preview window should expose at least one inland lake component after dry basin pruning, got {}",
            default_window_lakes.len()
        );
        assert!(
            map.sites
                .iter()
                .filter(|site| matches!(site.surface_kind, MacroSurfaceKind::LakeCandidate))
                .all(|site| site.ocean_basin.is_none()),
            "inland lake candidates must not retain ocean basin ownership"
        );
    }

    #[test]
    fn inland_water_components_are_not_all_lakes() {
        let patch = generate_voronoi_graph_patch(preview_like_request(42, 0, 0));
        let map = generate_macro_map(&patch, MacroMapConfig::new(42, 11));
        let dry_basin_sites = map
            .sites
            .iter()
            .filter(|site| matches!(site.surface_kind, MacroSurfaceKind::DryBasin))
            .count();

        assert!(
            dry_basin_sites > 0,
            "large closed inland depressions should be allowed to resolve as dry basins instead of all becoming lakes"
        );
    }

    #[test]
    fn closed_basin_policy_keeps_dry_and_wet_basin_roles_distinct() {
        let patch = generate_voronoi_graph_patch(preview_like_request(42, 0, 0));
        let map = generate_macro_map(&patch, MacroMapConfig::new(42, 11));
        let dry_basin_sites = map
            .sites
            .iter()
            .filter(|site| matches!(site.surface_kind, MacroSurfaceKind::DryBasin))
            .count();
        let wet_basin_sites = map
            .sites
            .iter()
            .filter(|site| {
                matches!(
                    site.surface_kind,
                    MacroSurfaceKind::LakeCandidate | MacroSurfaceKind::WetlandCandidate
                )
            })
            .count();

        assert!(
            dry_basin_sites > 0,
            "closed inland depressions should not all be promoted to standing water"
        );
        assert!(
            wet_basin_sites > 0,
            "closed inland depressions should not all become DryBasin"
        );
    }

    #[test]
    fn lake_component_size_policy_keeps_default_lakes_small() {
        let patch = generate_voronoi_graph_patch(preview_like_request(42, 0, 0));
        let map = generate_macro_map(&patch, MacroMapConfig::new(42, 11));
        let mut lake_sizes = HashMap::<MacroContinentId, usize>::new();

        for site in map.sites.iter().filter(|site| {
            matches!(
                site.surface_kind,
                MacroSurfaceKind::LakeCandidate | MacroSurfaceKind::WetlandCandidate
            )
        }) {
            if let Some(component) = site.continent {
                *lake_sizes.entry(component).or_default() += 1;
            }
        }

        assert!(
            lake_sizes.values().all(|&size| size <= 30),
            "launch lake policy should keep lake/wetland components within the documented rare-large soft cap: {:?}",
            lake_sizes
        );
    }

    #[test]
    fn default_preview_land_ratio_targets_land_majority_with_coastline() {
        let patch = generate_voronoi_graph_patch(preview_like_request(42, 0, 0));
        let map = generate_macro_map(&patch, MacroMapConfig::new(42, 11));
        let visible_sites = map
            .sites
            .iter()
            .filter(|site| in_default_preview_window(site.position))
            .collect::<Vec<_>>();
        let land_sites = visible_sites
            .iter()
            .filter(|site| site.surface_kind.is_land_owned())
            .count();
        let land_ratio = land_sites as f32 / visible_sites.len().max(1) as f32;
        let site_map = macro_sites_by_id(&map);
        let visible_coast_edges = map
            .coast_edges()
            .filter(|edge| {
                site_map
                    .get(&edge.sites[0])
                    .is_some_and(|site| in_default_preview_window(site.position))
                    || site_map
                        .get(&edge.sites[1])
                        .is_some_and(|site| in_default_preview_window(site.position))
            })
            .count();

        assert!(
            (0.57..=0.63).contains(&land_ratio),
            "default macro preview land ratio should lean toward 6:4 land/water, got {land_ratio:.3}"
        );
        assert!(
            visible_coast_edges >= 512,
            "coastline should remain complex after land bias tuning, got {visible_coast_edges} visible coast edges"
        );
    }

    #[test]
    fn small_stream_pocket_lakes_are_more_common_than_large_lakes() {
        let patch = generate_voronoi_graph_patch(preview_like_request(42, 0, 0));
        let map = generate_macro_map(&patch, MacroMapConfig::new(42, 11));
        let macro_sites = macro_sites_by_id(&map);
        let site_indices = patch
            .sites
            .iter()
            .enumerate()
            .map(|(index, site)| (site.id, index))
            .collect::<HashMap<_, _>>();
        let adjacency = site_adjacency(&patch, &site_indices);
        let lake_mask = patch
            .sites
            .iter()
            .map(|site| {
                in_default_preview_window(site.position)
                    && macro_sites.get(&site.id).is_some_and(|macro_site| {
                        matches!(macro_site.surface_kind, MacroSurfaceKind::LakeCandidate)
                    })
            })
            .collect::<Vec<_>>();
        let lake_sizes = connected_true_component_sizes(&adjacency, &lake_mask);

        let small_lakes = lake_sizes
            .iter()
            .filter(|&&size| (1..=4).contains(&size))
            .count();
        let large_lakes = lake_sizes.iter().filter(|&&size| size > 10).count();

        assert!(
            small_lakes >= 2,
            "seed 42 default preview should expose multiple 1..4 site stream-pocket lakes, got {lake_sizes:?}"
        );
        assert_eq!(
            large_lakes, 0,
            "large lakes should stay rare under the launch size policy: {lake_sizes:?}"
        );
    }

    #[test]
    fn dry_basin_sites_do_not_receive_ocean_coastness() {
        let patch = generate_voronoi_graph_patch(preview_like_request(42, 0, 0));
        let map = generate_macro_map(&patch, MacroMapConfig::new(42, 11));
        let dry_basin_sites = map
            .sites
            .iter()
            .filter(|site| matches!(site.surface_kind, MacroSurfaceKind::DryBasin))
            .collect::<Vec<_>>();

        assert!(!dry_basin_sites.is_empty());
        assert!(
            dry_basin_sites.iter().all(|site| site.coastness < 0.55),
            "dry basins are closed inland depressions and should not be coast-classified: {:?}",
            dry_basin_sites
                .iter()
                .map(|site| (site.id, site.coastness))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn sea_level_continentality_threshold_maps_to_zero_elevation() {
        let elevation = signed_macro_elevation_from_continentality_and_ruggedness(0.0, 0.95, 1.0);

        assert!(
            elevation.abs() <= 0.000_001,
            "continentality threshold should be the macro elevation sea level, got {elevation}"
        );
    }

    #[test]
    fn ruggedness_increases_neighbor_elevation_contrast() {
        let smooth_low =
            signed_macro_elevation_from_continentality_and_ruggedness(0.30, -0.45, 0.05);
        let smooth_high =
            signed_macro_elevation_from_continentality_and_ruggedness(0.30, 0.45, 0.05);
        let rugged_low =
            signed_macro_elevation_from_continentality_and_ruggedness(0.30, -0.45, 0.95);
        let rugged_high =
            signed_macro_elevation_from_continentality_and_ruggedness(0.30, 0.45, 0.95);
        let smooth_delta = (smooth_high - smooth_low).abs();
        let rugged_delta = (rugged_high - rugged_low).abs();

        assert!(
            rugged_delta > smooth_delta * 2.0,
            "ruggedness should amplify same-continentality local relief contrast: smooth_delta={smooth_delta} rugged_delta={rugged_delta}"
        );
    }

    #[test]
    fn land_macro_elevation_does_not_apply_coast_distance_recovery_profile() {
        let config = test_macro_config(404);
        let base = SiteContext {
            base_fields: GraphBaseFields::new(0.82, 0.45, 0.20, 0.92),
            ruggedness: 0.72,
            feature_hash: 1,
            is_land_owned: true,
            is_island_owned: false,
            is_tiny_local_minima_lake: false,
            is_inland_water: false,
            inland_water_surface: None,
            component_id: 10,
            graph_distance_to_coast: 0,
            spacing_blocks: DEFAULT_MACRO_GRAPH_DISTANCE_STEP_BLOCKS,
        };
        let coast = macro_field_sample_from_context(base, config);
        let inland = macro_field_sample_from_context(
            SiteContext {
                graph_distance_to_coast: 8,
                ..base
            },
            config,
        );

        assert!(
            (inland.signed_macro_elevation - coast.signed_macro_elevation).abs() <= 0.000_001,
            "identical graph fields should not change elevation only because coast distance changes: coast={} inland={}",
            coast.signed_macro_elevation,
            inland.signed_macro_elevation
        );
    }

    #[test]
    fn positive_land_elevation_is_not_long_range_coast_distance_curve() {
        let config = test_macro_config(405);
        let base = SiteContext {
            base_fields: GraphBaseFields::new(0.70, 0.42, 0.24, 0.64),
            ruggedness: 0.44,
            feature_hash: 2,
            is_land_owned: true,
            is_island_owned: false,
            is_tiny_local_minima_lake: false,
            is_inland_water: false,
            inland_water_surface: None,
            component_id: 11,
            graph_distance_to_coast: 0,
            spacing_blocks: DEFAULT_MACRO_GRAPH_DISTANCE_STEP_BLOCKS,
        };
        let recovered = macro_field_sample_from_context(
            SiteContext {
                graph_distance_to_coast: 4,
                ..base
            },
            config,
        );
        let far_inland = macro_field_sample_from_context(
            SiteContext {
                graph_distance_to_coast: 8,
                ..base
            },
            config,
        );

        assert!(
            (recovered.signed_macro_elevation - far_inland.signed_macro_elevation).abs()
                <= 0.000_001,
            "identical positive land context must not keep climbing with coast distance: recovered={} far={}",
            recovered.signed_macro_elevation,
            far_inland.signed_macro_elevation
        );
    }

    #[test]
    fn inland_lowland_can_be_lower_than_nearer_highland() {
        let config = test_macro_config(406);
        let highland = SiteContext {
            base_fields: GraphBaseFields::new(0.70, 0.34, 0.34, 0.82),
            ruggedness: 0.70,
            feature_hash: 3,
            is_land_owned: true,
            is_island_owned: false,
            is_tiny_local_minima_lake: false,
            is_inland_water: false,
            inland_water_surface: None,
            component_id: 12,
            graph_distance_to_coast: 2,
            spacing_blocks: DEFAULT_MACRO_GRAPH_DISTANCE_STEP_BLOCKS,
        };
        let lowland = SiteContext {
            base_fields: GraphBaseFields::new(0.70, 0.58, 0.30, -0.58),
            ruggedness: 0.10,
            graph_distance_to_coast: 8,
            ..highland
        };
        let near_highland = macro_field_sample_from_context(highland, config);
        let far_lowland = macro_field_sample_from_context(lowland, config);

        assert!(
            far_lowland.signed_macro_elevation < near_highland.signed_macro_elevation,
            "coast distance must not force inland lowlands above nearer highlands: lowland={} highland={}",
            far_lowland.signed_macro_elevation,
            near_highland.signed_macro_elevation
        );
    }

    #[test]
    fn coast_adjacent_land_deltas_are_not_systematically_steeper_than_inland_edges() {
        let patch = generate_voronoi_graph_patch(preview_like_request(
            42,
            -70 * DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            0,
        ));
        let map = generate_macro_map(&patch, test_macro_config(42));
        let site_map = macro_sites_by_id(&map);
        let mut coast_deltas = Vec::new();
        let mut inland_deltas = Vec::new();

        for edge in &map.edges {
            let Some(left) = site_map.get(&edge.sites[0]) else {
                continue;
            };
            let Some(right) = site_map.get(&edge.sites[1]) else {
                continue;
            };
            if left.surface_kind.is_land_owned() && right.surface_kind.is_land_owned() {
                let delta = (left.signed_macro_elevation - right.signed_macro_elevation).abs();
                let nearest_coast = left
                    .distance_to_coast_blocks
                    .min(right.distance_to_coast_blocks);

                if nearest_coast <= DEFAULT_MACRO_GRAPH_DISTANCE_STEP_BLOCKS {
                    coast_deltas.push(delta);
                } else if left.distance_to_coast_blocks
                    >= DEFAULT_MACRO_GRAPH_DISTANCE_STEP_BLOCKS * 2.0
                    && right.distance_to_coast_blocks
                        >= DEFAULT_MACRO_GRAPH_DISTANCE_STEP_BLOCKS * 2.0
                {
                    inland_deltas.push(delta);
                }
            }
        }

        let coast_stats = DistributionStats::from_values(coast_deltas);
        let inland_stats = DistributionStats::from_values(inland_deltas);
        eprintln!(
            "macro_map coast-adjacent-land/inland-land elevation deltas seed=42 cx=-70 cz=0 coast_adjacent={:?} inland={:?}",
            coast_stats, inland_stats
        );

        assert!(
            coast_stats.count >= 12,
            "not enough coast-adjacent land-land edges: {coast_stats:?}"
        );
        assert!(
            inland_stats.count >= 24,
            "not enough inland land-land edges: {inland_stats:?}"
        );
        assert!(
            coast_stats.median <= inland_stats.p75 * 1.35,
            "coast median delta should not be uniformly steeper than inland upper-quartile deltas: coast={coast_stats:?} inland={inland_stats:?}"
        );
        assert!(
            coast_stats.average <= inland_stats.average * 1.50,
            "coast average delta should not dominate inland average delta: coast={coast_stats:?} inland={inland_stats:?}"
        );
    }

    #[test]
    fn coastland_to_inland_land_deltas_do_not_dominate_inland_edges() {
        let patch = generate_voronoi_graph_patch(preview_like_request(
            42,
            -70 * DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            0,
        ));
        let map = generate_macro_map(&patch, test_macro_config(42));
        let site_map = macro_sites_by_id(&map);
        let mut coastland_transition_deltas = Vec::new();
        let mut inland_deltas = Vec::new();

        for edge in &map.edges {
            let Some(left) = site_map.get(&edge.sites[0]) else {
                continue;
            };
            let Some(right) = site_map.get(&edge.sites[1]) else {
                continue;
            };
            let delta = (left.signed_macro_elevation - right.signed_macro_elevation).abs();
            let left_coastland = left.surface_kind == MacroSurfaceKind::CoastLand;
            let right_coastland = right.surface_kind == MacroSurfaceKind::CoastLand;
            let left_inland_continent = left.surface_kind == MacroSurfaceKind::Continent;
            let right_inland_continent = right.surface_kind == MacroSurfaceKind::Continent;

            if (left_coastland && right_inland_continent)
                || (right_coastland && left_inland_continent)
            {
                coastland_transition_deltas.push(delta);
            } else if left_inland_continent
                && right_inland_continent
                && left.distance_to_coast_blocks >= DEFAULT_MACRO_GRAPH_DISTANCE_STEP_BLOCKS * 2.0
                && right.distance_to_coast_blocks >= DEFAULT_MACRO_GRAPH_DISTANCE_STEP_BLOCKS * 2.0
            {
                inland_deltas.push(delta);
            }
        }

        let coastland_stats = DistributionStats::from_values(coastland_transition_deltas);
        let inland_stats = DistributionStats::from_values(inland_deltas);
        eprintln!(
            "macro_map CoastLand-to-inland-continent/inland-continent elevation deltas seed=42 cx=-70 cz=0 coastland_transition={:?} inland={:?}",
            coastland_stats, inland_stats
        );

        assert!(
            coastland_stats.count >= 12,
            "not enough CoastLand-to-inland-continent edges: {coastland_stats:?}"
        );
        assert!(
            inland_stats.count >= 24,
            "not enough inland continent-continent edges: {inland_stats:?}"
        );
        assert!(
            coastland_stats.median <= inland_stats.p75 * 1.35,
            "CoastLand-to-inland-continent median delta should not be uniformly steeper than inland continent upper-quartile deltas: coastland={coastland_stats:?} inland={inland_stats:?}"
        );
        assert!(
            coastland_stats.average <= inland_stats.average * 1.50,
            "CoastLand-to-inland-continent average delta should not dominate inland continent average delta: coastland={coastland_stats:?} inland={inland_stats:?}"
        );
    }

    #[test]
    fn seeded_local_minima_like_sites_can_promote_small_stream_pocket_lakes() {
        let fields = GraphBaseFields::new(0.52, 0.72, 0.42, -0.42);

        assert!(small_stream_pocket_lake(fields, 0.10, 0.92, 0.08, 0));
        assert!(
            !small_stream_pocket_lake(fields, 0.62, 0.92, 0.08, 0),
            "coastal low spots should not become stream-pocket lakes"
        );
        assert!(
            !small_stream_pocket_lake(fields, 0.10, 0.62, 0.08, 0),
            "weak basinness should stay dry/ordinary land even with a favorable roll"
        );
        assert!(
            !small_stream_pocket_lake(fields, 0.10, 0.92, 0.08, 9_999),
            "promotion must remain a low-probability deterministic roll"
        );
    }

    #[test]
    fn tiny_local_minima_lake_roll_is_low_probability_and_size_limited() {
        assert_eq!(DEFAULT_TINY_LOCAL_MINIMA_LAKE_CHANCE_PER_10K, 3_600);
        assert_eq!(DEFAULT_TINY_LOCAL_MINIMA_LAKE_MAX_CHANCE_PER_10K, 4_000);
        assert!(tiny_local_minima_lake_roll(1, 0.70, 0));
        assert!(tiny_local_minima_lake_roll(3, 0.86, 3_899));
        assert!(
            !tiny_local_minima_lake_roll(3, 0.86, 4_000),
            "score bonus should stay capped instead of making tiny lakes too common"
        );
        assert!(
            !tiny_local_minima_lake_roll(4, 0.90, 0),
            "4+ cell basins should stay under the existing lake/wetland/dry basin policy"
        );
        assert!(
            !tiny_local_minima_lake_roll(1, 0.57, 0),
            "weak minima-like pockets should stay dry even with a favorable roll"
        );
        assert!(
            !tiny_local_minima_lake_roll(1, 0.86, 9_999),
            "promotion must be deterministic and low probability"
        );
    }

    #[test]
    fn tiny_local_minima_lake_mask_is_deterministic_land_owned_and_component_limited() {
        let found_promoted_seed = (1..160).find(|&seed| {
            let patch = generate_voronoi_graph_patch(test_request(seed, 0, 0));
            let site_indices = patch
                .sites
                .iter()
                .enumerate()
                .map(|(index, site)| (site.id, index))
                .collect::<HashMap<_, _>>();
            let adjacency = site_adjacency(&patch, &site_indices);
            let config = test_macro_config(seed);
            let land_mask = patch
                .sites
                .iter()
                .map(|site| is_land_base(site.base_fields, config))
                .collect::<Vec<_>>();
            let components = connected_components(&patch.sites, &adjacency, &land_mask);
            let water_component_sizes = component_sizes(&components);
            let ocean_components = explicit_ocean_components(
                &patch.sites,
                &components,
                &land_mask,
                &water_component_sizes,
            );
            let ocean_mask = components
                .iter()
                .enumerate()
                .map(|(index, component)| {
                    !land_mask[index] && ocean_components.get(component).copied().unwrap_or(false)
                })
                .collect::<Vec<_>>();
            let coast_distances = graph_distances_to_ocean_coast(&adjacency, &ocean_mask);
            let mask = tiny_local_minima_lake_mask(
                &patch.sites,
                &adjacency,
                &land_mask,
                &coast_distances,
                config,
            );
            let repeated = tiny_local_minima_lake_mask(
                &patch.sites,
                &adjacency,
                &land_mask,
                &coast_distances,
                config,
            );
            assert_eq!(
                mask, repeated,
                "tiny lake promotion must be seed deterministic"
            );
            assert!(
                mask.iter()
                    .enumerate()
                    .all(|(index, promoted)| !*promoted || land_mask[index]),
                "tiny local-minima lake promotion must not confuse ocean/water ownership"
            );
            let groups = connected_components(&patch.sites, &adjacency, &mask);
            let promoted_sizes = component_sizes(&groups);
            let component_limited = mask.iter().enumerate().all(|(index, promoted)| {
                !*promoted
                    || promoted_sizes.get(&groups[index]).is_some_and(|&size| {
                        (1..=DEFAULT_TINY_LOCAL_MINIMA_LAKE_MAX_CELLS).contains(&size)
                    })
            });
            assert!(
                component_limited,
                "tiny local-minima lakes should be limited to 1..=3 cells"
            );

            mask.iter().any(|promoted| *promoted)
        });

        assert!(
            found_promoted_seed.is_some(),
            "a bounded deterministic seed scan should find at least one tiny local-minima lake"
        );
    }

    #[test]
    fn macro_elevation_preserves_graph_elevation_order_on_same_ownership() {
        let found_ordered_pair = (32..96).any(|seed| {
            let patch = generate_voronoi_graph_patch(test_request(seed, 0, 0));
            let map = generate_macro_map(&patch, test_macro_config(seed));
            let macro_sites = macro_sites_by_id(&map);
            let mut land_sites = patch
                .sites
                .iter()
                .filter(|site| macro_sites[&site.id].surface_kind.is_land_owned())
                .collect::<Vec<_>>();
            land_sites.sort_by(|a, b| {
                a.base_fields
                    .elevation_seed
                    .total_cmp(&b.base_fields.elevation_seed)
            });

            let Some(low) = land_sites.first() else {
                return false;
            };
            let Some(high) = land_sites.last() else {
                return false;
            };

            macro_sites[&high.id].signed_macro_elevation
                > macro_sites[&low.id].signed_macro_elevation
        });

        assert!(found_ordered_pair);
    }

    #[test]
    fn land_bias_tunes_land_ownership() {
        let patch = generate_voronoi_graph_patch(test_request(7, 0, 0));
        let mut ocean_leaning = test_macro_config(7);
        ocean_leaning.land_bias = -0.24;
        let mut land_leaning = test_macro_config(7);
        land_leaning.land_bias = 0.24;

        let ocean_leaning_land_sites = generate_macro_map(&patch, ocean_leaning)
            .sites
            .iter()
            .filter(|site| site.surface_kind.is_land_owned())
            .count();
        let land_leaning_land_sites = generate_macro_map(&patch, land_leaning)
            .sites
            .iter()
            .filter(|site| site.surface_kind.is_land_owned())
            .count();

        assert!(
            land_leaning_land_sites > ocean_leaning_land_sites,
            "positive land_bias should increase land ownership: {land_leaning_land_sites} <= {ocean_leaning_land_sites}"
        );
    }

    #[test]
    fn coast_edges_are_land_ocean_boundaries() {
        let (map, site_map) = (7..64)
            .find_map(|seed| {
                let patch = generate_voronoi_graph_patch(test_request(seed, 0, 0));
                let map = generate_macro_map(&patch, test_macro_config(seed));
                let site_map = macro_sites_by_id(&map);
                let has_coast = map.coast_edges().next().is_some();

                has_coast.then_some((map, site_map))
            })
            .expect("expected at least one deterministic seed with a coast edge");

        for edge in map.coast_edges() {
            let a = site_map[&edge.sites[0]];
            let b = site_map[&edge.sites[1]];
            assert_ne!(
                a.surface_kind.is_land_owned(),
                b.surface_kind.is_land_owned()
            );
        }
    }

    #[test]
    fn corner_ocean_transition_edges_are_coast_guides_for_river_mouths() {
        let land_a = test_macro_site(
            1,
            Some(MacroContinentId(10)),
            0.08,
            0.16,
            0.80,
            0.0,
            0.0,
            0.0,
            0.0,
        );
        let land_b = test_macro_site(
            2,
            Some(MacroContinentId(10)),
            0.10,
            0.18,
            0.74,
            0.0,
            0.0,
            0.0,
            0.0,
        );

        let guide = macro_edge_guide_with_corner_kinds(
            Some(land_a),
            Some(land_b),
            Some([
                Some(MacroSurfaceKind::CoastLand),
                Some(MacroSurfaceKind::CoastOcean),
            ]),
            test_macro_config(123),
        );

        assert!(
            guide.is_coast,
            "river-mouth extension edges with ocean/non-ocean endpoint semantics must enter the canonical coast guide set"
        );
        assert_eq!(guide.coastness, 1.0);
    }

    #[test]
    fn lake_corner_transition_edges_do_not_become_coast_guides() {
        let lake = test_macro_site(
            1,
            Some(MacroContinentId(10)),
            -0.02,
            0.08,
            0.10,
            0.0,
            0.0,
            0.0,
            768.0,
        );
        let mut land = test_macro_site(
            2,
            Some(MacroContinentId(10)),
            0.10,
            0.18,
            0.10,
            0.0,
            0.0,
            0.0,
            768.0,
        );
        let mut lake = lake;
        lake.surface_kind = MacroSurfaceKind::LakeCandidate;
        land.surface_kind = MacroSurfaceKind::Continent;

        let guide = macro_edge_guide_with_corner_kinds(
            Some(lake),
            Some(land),
            Some([
                Some(MacroSurfaceKind::LakeCandidate),
                Some(MacroSurfaceKind::Continent),
            ]),
            test_macro_config(124),
        );

        assert!(
            !guide.is_coast,
            "lake/non-lake endpoint semantics must not be promoted into ocean coast guides"
        );
    }

    #[test]
    fn river_candidates_are_not_selected_in_stage_three_macro_map() {
        let patch = generate_voronoi_graph_patch(test_request(91, 0, 0));
        let map = generate_macro_map(&patch, test_macro_config(91));

        assert_eq!(map.river_candidate_edges().count(), 0);
    }

    #[test]
    fn lake_edge_classification_uses_sites_and_corners() {
        let patch = generate_voronoi_graph_patch(preview_like_request(42, 0, 0));
        let map = generate_macro_map(&patch, MacroMapConfig::new(42, 11));
        let lake_edges = map
            .edges
            .iter()
            .filter(|edge| edge.lake_class.excludes_selected_river())
            .count();
        let boundary_edges = map
            .edges
            .iter()
            .filter(|edge| edge.lake_class == MacroLakeEdgeClass::LakeBoundary)
            .count();

        assert!(
            lake_edges > 0,
            "seed 42 preview-sized patch should expose explicit lake edge classes"
        );
        assert!(
            boundary_edges > 0,
            "lake edge classification should distinguish visible lake boundaries"
        );
    }

    #[test]
    fn macro_map_resolves_biome_cells_for_sites_before_macro_field() {
        let patch = generate_voronoi_graph_patch(preview_like_request(42, 0, 0));
        let map = generate_macro_map(&patch, MacroMapConfig::new(42, 11));

        assert_eq!(map.biomes.len(), map.sites.len());
        assert!(map.sites.iter().all(|site| {
            map.biome(site.id)
                .is_some_and(|biome| biome.site == site.id && biome.context.coastness.is_finite())
        }));
        assert!(map.sites.iter().all(|site| {
            map.biome(site.id)
                .is_some_and(|biome| (biome.context.ruggedness - site.ridgeness).abs() <= 0.000_001)
        }));
        assert!(
            map.biomes.iter().any(|biome| matches!(
                biome.biome,
                super::super::biome::GraphBiomeKind::ShallowOcean
                    | super::super::biome::GraphBiomeKind::DeepOcean
            )),
            "preview-sized macro map should carry ocean biome classification"
        );
    }

    #[test]
    fn biome_lake_cells_match_macro_lake_candidates() {
        let mut saw_lake_candidate = false;

        for seed in 1..=128 {
            let patch = generate_voronoi_graph_patch(test_request(seed, 0, 0));
            let map = generate_macro_map(&patch, test_macro_config(seed));
            let biomes_by_site = map
                .biomes
                .iter()
                .map(|biome| (biome.site, biome))
                .collect::<HashMap<_, _>>();

            assert_eq!(map.biomes.len(), map.sites.len());

            for site in &map.sites {
                let biome = biomes_by_site
                    .get(&site.id)
                    .copied()
                    .expect("each macro site must have a matching biome cell");
                let macro_lake = site.surface_kind == MacroSurfaceKind::LakeCandidate;
                let biome_lake_role = biome.context.water_role == GraphBiomeWaterRole::Lake;
                let biome_lake_kind = biome.biome == GraphBiomeKind::Lake;

                saw_lake_candidate |= macro_lake;

                assert_eq!(
                    macro_lake, biome_lake_role,
                    "GraphBiomeWaterRole::Lake must mirror MacroSurfaceKind::LakeCandidate for site {:?}",
                    site.id
                );
                assert_eq!(
                    macro_lake, biome_lake_kind,
                    "GraphBiomeKind::Lake must mirror MacroSurfaceKind::LakeCandidate for site {:?}",
                    site.id
                );
            }
        }

        assert!(
            saw_lake_candidate,
            "bounded deterministic seed scan should include at least one lake candidate"
        );
    }

    #[test]
    fn bounded_seed_scan_exposes_expected_land_biome_distribution() {
        let mut counts = HashMap::<GraphBiomeKind, usize>::new();

        for seed in 1..=128 {
            let patch = generate_voronoi_graph_patch(test_request(seed, 0, 0));
            let map = generate_macro_map(&patch, test_macro_config(seed));

            for biome in &map.biomes {
                *counts.entry(biome.biome).or_default() += 1;
            }
        }

        let expected_groups: &[(&str, &[GraphBiomeKind])] = &[
            ("temperate grassland", &[GraphBiomeKind::TemperateGrassland]),
            (
                "dry/arid",
                &[
                    GraphBiomeKind::Desert,
                    GraphBiomeKind::SemiDesert,
                    GraphBiomeKind::Steppe,
                    GraphBiomeKind::DryShrubland,
                    GraphBiomeKind::MediterraneanShrubland,
                ],
            ),
            (
                "tropical",
                &[
                    GraphBiomeKind::Savanna,
                    GraphBiomeKind::TropicalDryForest,
                    GraphBiomeKind::MonsoonForest,
                    GraphBiomeKind::TropicalRainforest,
                ],
            ),
            (
                "alpine",
                &[
                    GraphBiomeKind::SubalpineWoodland,
                    GraphBiomeKind::AlpineMeadow,
                ],
            ),
        ];

        for (label, expected) in expected_groups {
            assert!(
                expected
                    .iter()
                    .any(|biome| counts.get(biome).copied().unwrap_or(0) > 0),
                "{label} biome branch should appear in a bounded deterministic seed scan; counts={counts:?}"
            );
        }
    }

    #[test]
    fn alpine_sites_are_high_elevation_mountain_sites() {
        let mut alpine_count = 0;

        for seed in 1..=128 {
            let patch = generate_voronoi_graph_patch(test_request(seed, 0, 0));
            let map = generate_macro_map(&patch, test_macro_config(seed));

            for biome in &map.biomes {
                if biome.biome == GraphBiomeKind::AlpineMeadow {
                    alpine_count += 1;
                    assert!(
                        biome.context.elevation >= 0.56,
                        "AlpineMeadow should be restricted to higher macro elevation: {:?}",
                        biome.context
                    );
                    assert!(
                        biome.context.mountainness >= 0.34,
                        "AlpineMeadow should keep mountain context: {:?}",
                        biome.context
                    );
                    assert!(
                        biome.context.ruggedness >= 0.10,
                        "AlpineMeadow should keep rugged mountain context: {:?}",
                        biome.context
                    );
                }
            }
        }

        assert!(
            alpine_count > 0,
            "bounded deterministic seed scan should find high-elevation AlpineMeadow cells"
        );
    }

    #[test]
    fn graph_biome_water_role_splits_shallow_and_deep_ocean() {
        assert_eq!(
            graph_biome_water_role(MacroSurfaceKind::OceanBasin, -0.08, 0.70),
            GraphBiomeWaterRole::ShallowOcean
        );
        assert_eq!(
            graph_biome_water_role(MacroSurfaceKind::OceanBasin, -0.64, 0.0),
            GraphBiomeWaterRole::DeepOcean
        );
    }

    #[test]
    fn graph_biome_water_role_preserves_coast_and_wetland_meaning() {
        assert_eq!(
            graph_biome_water_role(MacroSurfaceKind::CoastLand, 0.02, 0.90),
            GraphBiomeWaterRole::Coast
        );
        assert_eq!(
            graph_biome_water_role(MacroSurfaceKind::WetlandCandidate, 0.04, 0.10),
            GraphBiomeWaterRole::Wetland
        );
    }

    #[test]
    fn ridge_candidates_require_component_interior_gradient_and_divide_context() {
        let component = Some(MacroContinentId(10));
        let lowland = test_macro_site(1, component, 0.74, 0.75, 0.95, 0.08, 0.06, 0.20, 768.0);
        let high_plain = test_macro_site(2, component, 0.78, 0.78, 0.95, 0.08, 0.06, 0.18, 768.0);
        let ridge_a = test_macro_site(3, component, 0.42, 0.78, 0.18, 0.82, 0.78, 0.18, 768.0);
        let ridge_b = test_macro_site(4, component, 0.64, 0.84, 0.12, 0.86, 0.82, 0.16, 768.0);
        let coastal_ridge = test_macro_site(5, component, 0.66, 0.84, 0.96, 0.90, 0.86, 0.12, 0.0);
        let other_component = test_macro_site(
            6,
            Some(MacroContinentId(11)),
            0.66,
            0.84,
            0.10,
            0.90,
            0.86,
            0.12,
            768.0,
        );
        let config = test_macro_config(123);

        assert!(!macro_edge_guide(Some(lowland), Some(high_plain), config).is_ridge_candidate);
        assert!(macro_edge_guide(Some(ridge_a), Some(ridge_b), config).is_ridge_candidate);
        assert!(
            macro_edge_guide(Some(ridge_a), Some(ridge_b), config).drainage_divide_potential
                >= 0.50
        );
        assert!(!macro_edge_guide(Some(ridge_a), Some(coastal_ridge), config).is_ridge_candidate);
        assert!(!macro_edge_guide(Some(ridge_a), Some(other_component), config).is_ridge_candidate);
    }

    #[test]
    fn fault_candidates_preserve_signed_gradient_context() {
        let component = Some(MacroContinentId(20));
        let left = test_macro_site(7, component, 0.26, 0.68, 0.18, 0.56, 0.44, 0.24, 576.0);
        let right = test_macro_site(8, component, 0.58, 0.72, 0.14, 0.62, 0.50, 0.22, 576.0);

        let guide = macro_edge_guide(Some(left), Some(right), test_macro_config(124));

        assert!(guide.is_fault_candidate);
        assert!(guide.signed_elevation_gradient > 0.0);
        assert!(guide.mountainness >= 0.38);
    }

    #[test]
    fn adjacent_patch_overlap_keeps_macro_sites_and_edges_stable() {
        let left = generate_voronoi_graph_patch(test_request(77, 0, 0));
        let right =
            generate_voronoi_graph_patch(test_request(77, DEFAULT_GRAPH_REGION_SIZE_BLOCKS, 0));
        let config = test_macro_config(77);
        let left_map = generate_macro_map(&left, config);
        let right_map = generate_macro_map(&right, config);
        let overlap_region = GraphRegionCoord::new(1, 0);
        let left_sites = macro_sites_in_region_by_id(&left_map, overlap_region);
        let right_sites = macro_sites_in_region_by_id(&right_map, overlap_region);
        let left_edges = internal_macro_edges_by_id(&left_map, &left_sites);
        let right_edges = internal_macro_edges_by_id(&right_map, &right_sites);

        assert!(!left_sites.is_empty());
        assert_eq!(left_sites, right_sites);
        assert!(!left_edges.is_empty());
        assert_eq!(left_edges, right_edges);
    }

    fn test_request(
        seed: u64,
        center_world_x: i32,
        center_world_z: i32,
    ) -> VoronoiGraphPatchRequest {
        VoronoiGraphPatchRequest::new(
            VoronoiGraphConfig {
                seed,
                generator_version: 3,
                region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
                site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
                padding_regions: 1,
            },
            center_world_x,
            center_world_z,
        )
    }

    fn preview_like_request(
        seed: u64,
        center_world_x: i32,
        center_world_z: i32,
    ) -> VoronoiGraphPatchRequest {
        VoronoiGraphPatchRequest::new(
            VoronoiGraphConfig {
                seed,
                generator_version: 11,
                region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
                site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
                padding_regions: 17,
            },
            center_world_x,
            center_world_z,
        )
    }

    fn test_macro_config(seed: u64) -> MacroMapConfig {
        MacroMapConfig {
            seed,
            generator_version: 3,
            sea_level: 0.0,
            coast_width_blocks: 192.0,
            ridge_candidate_threshold: DEFAULT_MACRO_RIDGE_CANDIDATE_THRESHOLD,
            river_candidate_threshold: DEFAULT_MACRO_RIVER_CANDIDATE_THRESHOLD,
            land_bias: DEFAULT_MACRO_LAND_BIAS,
        }
    }

    fn macro_sites_by_id(map: &GraphMacroMap) -> HashMap<VoronoiSiteId, MacroSite> {
        map.sites.iter().map(|site| (site.id, *site)).collect()
    }

    fn connected_true_component_sizes(adjacency: &[Vec<usize>], mask: &[bool]) -> Vec<usize> {
        let mut sizes = Vec::new();
        let mut visited = vec![false; mask.len()];

        for start in 0..mask.len() {
            if visited[start] || !mask[start] {
                continue;
            }

            let mut stack = vec![start];
            let mut size = 0;
            visited[start] = true;

            while let Some(index) = stack.pop() {
                size += 1;
                for &neighbor in &adjacency[index] {
                    if visited[neighbor] || !mask[neighbor] {
                        continue;
                    }
                    visited[neighbor] = true;
                    stack.push(neighbor);
                }
            }

            sizes.push(size);
        }

        sizes
    }

    fn in_default_preview_window(position: WorldPlanePoint) -> bool {
        position.x >= -16_384.0
            && position.x <= 16_384.0
            && position.z >= -9_216.0
            && position.z <= 9_216.0
    }

    fn macro_sites_in_region_by_id(
        map: &GraphMacroMap,
        region: GraphRegionCoord,
    ) -> HashMap<VoronoiSiteId, MacroSite> {
        map.sites
            .iter()
            .filter(|site| site.owner_region == region)
            .map(|site| (site.id, *site))
            .collect()
    }

    fn internal_macro_edges_by_id(
        map: &GraphMacroMap,
        sites: &HashMap<VoronoiSiteId, MacroSite>,
    ) -> HashMap<VoronoiEdgeId, MacroEdge> {
        map.edges
            .iter()
            .filter(|edge| sites.contains_key(&edge.sites[0]) && sites.contains_key(&edge.sites[1]))
            .map(|edge| (edge.id, *edge))
            .collect()
    }

    #[derive(Debug)]
    struct DistributionStats {
        count: usize,
        average: f32,
        median: f32,
        p75: f32,
    }

    impl DistributionStats {
        fn from_values(mut values: Vec<f32>) -> Self {
            values.sort_by(|left, right| left.total_cmp(right));
            let count = values.len();
            let average = if count == 0 {
                0.0
            } else {
                values.iter().sum::<f32>() / count as f32
            };

            Self {
                count,
                average,
                median: percentile(&values, 0.50),
                p75: percentile(&values, 0.75),
            }
        }
    }

    fn percentile(sorted_values: &[f32], percentile: f32) -> f32 {
        if sorted_values.is_empty() {
            return 0.0;
        }

        let index =
            ((sorted_values.len() - 1) as f32 * percentile.clamp(0.0, 1.0)).round() as usize;
        sorted_values[index]
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 0.000_001,
            "actual={actual} expected={expected}"
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn test_macro_site(
        id: u64,
        continent: Option<MacroContinentId>,
        signed_macro_elevation: f32,
        continentality: f32,
        coastness: f32,
        mountainness: f32,
        ridgeness: f32,
        basinness: f32,
        distance_to_coast_blocks: f32,
    ) -> MacroSite {
        MacroSite {
            id: VoronoiSiteId(id),
            owner_region: GraphRegionCoord::new(0, 0),
            position: WorldPlanePoint::new(0.0, 0.0),
            surface_kind: MacroSurfaceKind::Continent,
            continent,
            ocean_basin: None,
            signed_macro_elevation,
            continentality,
            coastness,
            distance_to_coast_blocks,
            distance_to_continent_core_blocks: distance_to_coast_blocks,
            distance_to_ocean_basin_blocks: distance_to_coast_blocks,
            mountainness,
            ridgeness,
            basinness,
        }
    }
}
