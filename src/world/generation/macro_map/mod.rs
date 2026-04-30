use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

use super::graph::{
    GraphBaseFields, GraphRegionCoord, VoronoiCornerId, VoronoiEdgeId, VoronoiGraphPatch,
    VoronoiSiteId, WorldPlanePoint,
};

pub const DEFAULT_MACRO_SUPER_CELL_SIZE_BLOCKS: i32 = 4096;
pub const DEFAULT_MACRO_COAST_WIDTH_BLOCKS: f32 = 384.0;
pub const DEFAULT_MACRO_RIDGE_CANDIDATE_THRESHOLD: f32 = 0.58;
pub const DEFAULT_MACRO_RIVER_CANDIDATE_THRESHOLD: f32 = 0.66;
pub const DEFAULT_MACRO_LAND_BIAS: f32 = 0.0;
pub const DEFAULT_MACRO_ISLAND_STRENGTH: f32 = 0.40;

const HASH_CONTINENT_FIELD: u64 = 0x2179_c56a_5bb7_43d1;
const HASH_OCEAN_FIELD: u64 = 0x6c8e_9cf5_12a4_f0b3;
const HASH_MOUNTAIN_FIELD: u64 = 0xb451_2d4e_9f07_63bb;
const HASH_COAST_WARP_FIELD: u64 = 0x7b7b_6a55_c04a_57c1;
const HASH_ISLAND_FIELD: u64 = 0x15a1_1d5e_7a11_9c31;
const CORE_SEARCH_RADIUS_CELLS: i32 = 5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MacroMapConfig {
    pub seed: u64,
    pub generator_version: u32,
    pub super_cell_size_blocks: i32,
    pub sea_level: f32,
    pub coast_width_blocks: f32,
    pub ridge_candidate_threshold: f32,
    pub river_candidate_threshold: f32,
    pub land_bias: f32,
    pub island_strength: f32,
}

impl MacroMapConfig {
    pub const fn new(seed: u64, generator_version: u32) -> Self {
        Self {
            seed,
            generator_version,
            super_cell_size_blocks: DEFAULT_MACRO_SUPER_CELL_SIZE_BLOCKS,
            sea_level: 0.0,
            coast_width_blocks: DEFAULT_MACRO_COAST_WIDTH_BLOCKS,
            ridge_candidate_threshold: DEFAULT_MACRO_RIDGE_CANDIDATE_THRESHOLD,
            river_candidate_threshold: DEFAULT_MACRO_RIVER_CANDIDATE_THRESHOLD,
            land_bias: DEFAULT_MACRO_LAND_BIAS,
            island_strength: DEFAULT_MACRO_ISLAND_STRENGTH,
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
    OceanBasin,
    CoastLand,
    CoastOcean,
    LakeCandidate,
    WetlandCandidate,
}

impl MacroSurfaceKind {
    pub const fn is_land_owned(self) -> bool {
        matches!(
            self,
            Self::Continent | Self::CoastLand | Self::LakeCandidate | Self::WetlandCandidate
        )
    }

    pub const fn is_ocean_owned(self) -> bool {
        matches!(self, Self::OceanBasin | Self::CoastOcean)
    }

    pub const fn is_coast(self) -> bool {
        matches!(self, Self::CoastLand | Self::CoastOcean)
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
    pub ridgeness: f32,
    pub river_potential: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MacroEdge {
    pub id: VoronoiEdgeId,
    pub sites: [VoronoiSiteId; 2],
    pub corners: [VoronoiCornerId; 2],
    pub guide: MacroEdgeGuide,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GraphMacroMap {
    pub sites: Vec<MacroSite>,
    pub corners: Vec<MacroCorner>,
    pub edges: Vec<MacroEdge>,
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

    pub fn coast_edges(&self) -> impl Iterator<Item = &MacroEdge> {
        self.edges.iter().filter(|edge| edge.guide.is_coast)
    }

    pub fn ridge_candidate_edges(&self) -> impl Iterator<Item = &MacroEdge> {
        self.edges
            .iter()
            .filter(|edge| edge.guide.is_ridge_candidate)
    }

    pub fn river_candidate_edges(&self) -> impl Iterator<Item = &MacroEdge> {
        self.edges
            .iter()
            .filter(|edge| edge.guide.is_river_candidate)
    }
}

pub fn generate_macro_map(patch: &VoronoiGraphPatch, config: MacroMapConfig) -> GraphMacroMap {
    validate_macro_map_config(config);

    let mut sites = patch
        .sites
        .par_iter()
        .map(|site| {
            let sample =
                sample_macro_fields(site.position, site.base_fields, site.ruggedness, config);
            MacroSite {
                id: site.id,
                owner_region: site.owner_region,
                position: site.position,
                surface_kind: sample.surface_kind,
                continent: sample.continent,
                ocean_basin: sample.ocean_basin,
                signed_macro_elevation: sample.signed_macro_elevation,
                continentality: sample.continentality,
                coastness: sample.coastness,
                distance_to_coast_blocks: sample.distance_to_coast_blocks,
                distance_to_continent_core_blocks: sample.distance_to_continent_core_blocks,
                distance_to_ocean_basin_blocks: sample.distance_to_ocean_basin_blocks,
                mountainness: sample.mountainness,
                ridgeness: sample.ridgeness,
                basinness: sample.basinness,
            }
        })
        .collect::<Vec<_>>();
    sites.sort_by_key(|site| site.id.0);

    let mut corners = patch
        .corners
        .par_iter()
        .map(|corner| {
            let sample = sample_macro_fields(corner.position, corner.base_fields, 0.5, config);
            MacroCorner {
                id: corner.id,
                position: corner.position,
                surface_kind: sample.surface_kind,
                continent: sample.continent,
                ocean_basin: sample.ocean_basin,
                signed_macro_elevation: sample.signed_macro_elevation,
                continentality: sample.continentality,
                coastness: sample.coastness,
                distance_to_coast_blocks: sample.distance_to_coast_blocks,
                mountainness: sample.mountainness,
                ridgeness: sample.ridgeness,
                basinness: sample.basinness,
            }
        })
        .collect::<Vec<_>>();
    corners.sort_by_key(|corner| corner.id.0);

    let site_map = sites
        .iter()
        .map(|site| (site.id, *site))
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
                guide: macro_edge_guide(a, b, edge.hydrology_bias, config),
            }
        })
        .collect::<Vec<_>>();
    edges.sort_by_key(|edge| edge.id.0);
    apply_pre_hydrology_river_corridors(&mut edges, &site_map, config);

    GraphMacroMap {
        sites,
        corners,
        edges,
    }
}

#[derive(Debug, Clone, Copy)]
struct MacroFieldSample {
    surface_kind: MacroSurfaceKind,
    continent: Option<MacroContinentId>,
    ocean_basin: Option<MacroOceanBasinId>,
    signed_macro_elevation: f32,
    continentality: f32,
    coastness: f32,
    distance_to_coast_blocks: f32,
    distance_to_continent_core_blocks: f32,
    distance_to_ocean_basin_blocks: f32,
    mountainness: f32,
    ridgeness: f32,
    basinness: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MacroCoreKind {
    Continent,
    Ocean,
}

#[derive(Debug, Clone, Copy)]
struct MacroCore {
    id: u64,
    position: WorldPlanePoint,
}

fn validate_macro_map_config(config: MacroMapConfig) {
    assert!(
        config.super_cell_size_blocks > 0,
        "super_cell_size_blocks must be positive"
    );
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
    assert!(
        config.island_strength.is_finite() && config.island_strength >= 0.0,
        "island_strength must be non-negative and finite"
    );
}

fn sample_macro_fields(
    position: WorldPlanePoint,
    base_fields: GraphBaseFields,
    ruggedness: f32,
    config: MacroMapConfig,
) -> MacroFieldSample {
    let warped_position = warped_macro_position(position, config);
    let continent_core = nearest_macro_core(warped_position, MacroCoreKind::Continent, config);
    let ocean_core = nearest_macro_core(warped_position, MacroCoreKind::Ocean, config);
    let continent_distance = distance(warped_position, continent_core.position);
    let ocean_distance = distance(warped_position, ocean_core.position);
    let normalized_core_balance = ((ocean_distance - continent_distance)
        / config.super_cell_size_blocks as f32)
        .clamp(-1.0, 1.0);
    let large_landmass = continent_field(warped_position, config);
    let islandness = island_field(position, large_landmass, config);
    let continentality = clamp_signed(
        normalized_core_balance * 0.46
            + large_landmass * 0.38
            + islandness * config.island_strength
            + base_fields.continentality * 0.18
            + base_fields.elevation_seed * 0.05
            + config.land_bias
            - config.sea_level,
    );
    let is_land_owned = continentality >= 0.0;
    let distance_to_coast_blocks =
        ((ocean_distance - continent_distance).abs() * 0.5).min(config.coast_width_blocks * 4.0);
    let coastness = (1.0 - distance_to_coast_blocks / config.coast_width_blocks).clamp(0.0, 1.0);
    let ridge_wave = deterministic_ridge_wave(position, config);
    let highlandness = smoothstep(0.06, 0.64, continentality.max(0.0) + ridge_wave * 0.42);
    let mountainness = if is_land_owned {
        smoothstep(
            0.34,
            0.90,
            ridge_wave * 0.52
                + ruggedness.clamp(0.0, 1.0) * 0.18
                + continentality.max(0.0) * 0.18
                + highlandness * 0.10
                + (1.0 - coastness) * 0.02,
        )
    } else {
        0.0
    };
    let ridgeness = if is_land_owned {
        smoothstep(
            0.45,
            0.90,
            ridge_wave * 0.52
                + mountainness * 0.28
                + base_fields.elevation_seed.max(0.0) * 0.12
                + ruggedness.clamp(0.0, 1.0) * 0.08,
        )
    } else {
        0.0
    };
    let basinness = if is_land_owned {
        clamp_unit(
            (1.0 - mountainness) * 0.34
                + (1.0 - base_fields.elevation_seed.max(0.0)) * 0.23
                + base_fields.hydration * 0.23
                + coastness * 0.20,
        )
    } else {
        clamp_unit((-continentality).max(0.0) * 0.72 + (1.0 - coastness) * 0.28)
    };
    let mut signed_macro_elevation = if is_land_owned {
        0.04 + continentality.max(0.0) * 0.58
            + base_fields.elevation_seed * 0.14
            + mountainness * 0.24
            + ridgeness * 0.20
            - basinness * 0.16
    } else {
        -0.04 + continentality.min(0.0) * 0.62 + base_fields.elevation_seed * 0.08
            - basinness * 0.18
    };
    signed_macro_elevation = if is_land_owned {
        signed_macro_elevation.max(0.01)
    } else {
        signed_macro_elevation.min(-0.01)
    };

    MacroFieldSample {
        surface_kind: surface_kind(is_land_owned, coastness, basinness, signed_macro_elevation),
        continent: is_land_owned.then_some(MacroContinentId(continent_core.id)),
        ocean_basin: (!is_land_owned).then_some(MacroOceanBasinId(ocean_core.id)),
        signed_macro_elevation: signed_macro_elevation.clamp(-1.0, 1.5),
        continentality,
        coastness,
        distance_to_coast_blocks,
        distance_to_continent_core_blocks: continent_distance,
        distance_to_ocean_basin_blocks: ocean_distance,
        mountainness,
        ridgeness,
        basinness,
    }
}

fn surface_kind(
    is_land_owned: bool,
    coastness: f32,
    basinness: f32,
    signed_macro_elevation: f32,
) -> MacroSurfaceKind {
    if is_land_owned {
        if coastness >= 0.55 {
            MacroSurfaceKind::CoastLand
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

fn macro_edge_guide(
    a: Option<MacroSite>,
    b: Option<MacroSite>,
    hydrology_bias: f32,
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
    let is_coast = a_land != b_land;
    let coastness = if is_coast {
        1.0
    } else {
        ((a.coastness + b.coastness) * 0.5).clamp(0.0, 1.0)
    };
    let average_elevation = (a.signed_macro_elevation + b.signed_macro_elevation) * 0.5;
    let elevation_slope = (a.signed_macro_elevation - b.signed_macro_elevation).abs();
    let both_land = a_land && b_land;
    let highland_edge = smoothstep(0.18, 0.78, average_elevation);
    let ridgeness = (((a.ridgeness + b.ridgeness) * 0.5) * 0.62
        + ((a.mountainness + b.mountainness) * 0.5) * 0.23
        + highland_edge * 0.15)
        .clamp(0.0, 1.0);
    let is_ridge_candidate = both_land
        && ridgeness >= config.ridge_candidate_threshold
        && average_elevation > 0.12
        && a.coastness.max(b.coastness) < 0.92;
    let average_basinness = (a.basinness + b.basinness) * 0.5;
    let average_mountainness = (a.mountainness + b.mountainness) * 0.5;
    let average_coastness = (a.coastness + b.coastness) * 0.5;
    let river_potential = if both_land {
        (average_basinness * 0.30
            + (1.0 - ridgeness) * 0.14
            + (1.0 - average_mountainness) * 0.10
            + hydrology_bias.clamp(0.0, 1.0) * 0.26
            + smoothstep(0.02, 0.26, elevation_slope) * 0.20)
            .clamp(0.0, 1.0)
    } else {
        0.0
    };
    let is_river_candidate = both_land
        && !is_ridge_candidate
        && river_potential >= config.river_candidate_threshold
        && average_basinness >= 0.46
        && average_coastness <= 0.82
        && elevation_slope >= 0.018
        && average_elevation > 0.02;
    let is_fault_candidate =
        both_land && is_ridge_candidate && elevation_slope > 0.18 && hydrology_bias > 0.40;

    MacroEdgeGuide {
        is_coast,
        is_ridge_candidate,
        is_river_candidate,
        is_fault_candidate,
        coastness,
        ridgeness,
        river_potential,
    }
}

fn apply_pre_hydrology_river_corridors(
    edges: &mut [MacroEdge],
    sites: &HashMap<VoronoiSiteId, MacroSite>,
    config: MacroMapConfig,
) {
    let adjacency = river_corridor_adjacency(edges, sites);
    let mut sources = sites
        .values()
        .filter(|site| is_river_corridor_source(site, config))
        .map(|site| (river_corridor_source_score(site, config), site.id))
        .collect::<Vec<_>>();
    sources.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.0.cmp(&b.1.0)));

    let max_sources = (sources.len() / 8).clamp(8, 96);
    let max_steps = ((config.super_cell_size_blocks as f32 / 64.0).round() as usize).clamp(12, 48);
    let minimum_edge_score = (config.river_candidate_threshold * 0.70).clamp(0.28, 0.86);
    let mut selected_edges = HashSet::new();

    for (_, source_id) in sources.into_iter().take(max_sources) {
        let mut current_id = source_id;
        let mut visited_sites = HashSet::from([source_id]);

        for _ in 0..max_steps {
            let Some(current) = sites.get(&current_id).copied() else {
                break;
            };
            if !current.surface_kind.is_land_owned() || current.signed_macro_elevation <= -0.01 {
                break;
            }

            let Some(next) = best_downstream_corridor_step(
                current,
                &adjacency,
                sites,
                &visited_sites,
                minimum_edge_score,
                config,
            ) else {
                break;
            };

            selected_edges.insert(next.edge_id);
            current_id = next.site_id;
            visited_sites.insert(current_id);
        }
    }
    selected_edges.extend(pre_hydrology_corner_edge_corridors(edges, sites, config));
    selected_edges.extend(river_corridor_outlet_extensions(
        edges,
        sites,
        &selected_edges,
        config,
    ));

    for edge in edges {
        if !selected_edges.contains(&edge.id) {
            edge.guide.is_river_candidate = false;
            continue;
        }

        edge.guide.is_river_candidate = true;
        edge.guide.river_potential = edge.guide.river_potential.max(0.72);
    }
}

fn pre_hydrology_corner_edge_corridors(
    edges: &[MacroEdge],
    sites: &HashMap<VoronoiSiteId, MacroSite>,
    config: MacroMapConfig,
) -> HashSet<VoronoiEdgeId> {
    let mut corner_edges = HashMap::<VoronoiCornerId, Vec<usize>>::new();

    for (index, edge) in edges.iter().enumerate() {
        if !is_river_corridor_edge_candidate(*edge, sites) {
            continue;
        }

        corner_edges.entry(edge.corners[0]).or_default().push(index);
        corner_edges.entry(edge.corners[1]).or_default().push(index);
    }

    for indices in corner_edges.values_mut() {
        indices.sort_by_key(|index| edges[*index].id.0);
        indices.dedup();
    }

    let max_steps = ((config.super_cell_size_blocks as f32 / 128.0).round() as usize).clamp(8, 32);
    let mut selected = HashSet::new();

    for source_index in 0..edges.len() {
        if !is_river_corridor_edge_candidate(edges[source_index], sites) {
            continue;
        }
        let Some(source_score) =
            river_corridor_edge_source_score(edges[source_index], sites, config)
        else {
            continue;
        };
        if source_score < 0.55
            || deterministic_corridor_edge_bias(edges[source_index].id, config) > 0.08
        {
            continue;
        }

        let mut current_index = source_index;
        let mut visited = HashSet::from([edges[current_index].id]);

        for _ in 0..max_steps {
            let current = edges[current_index];
            selected.insert(current.id);

            if current.guide.is_coast || river_corridor_edge_drainage(current, sites) <= -0.01 {
                break;
            }

            let Some(next_index) = best_corner_connected_downstream_edge(
                current_index,
                edges,
                sites,
                &corner_edges,
                &visited,
            ) else {
                break;
            };

            current_index = next_index;
            visited.insert(edges[current_index].id);
        }
    }

    selected
}

fn river_corridor_outlet_extensions(
    edges: &[MacroEdge],
    sites: &HashMap<VoronoiSiteId, MacroSite>,
    selected_edges: &HashSet<VoronoiEdgeId>,
    config: MacroMapConfig,
) -> HashSet<VoronoiEdgeId> {
    let edge_map = edges
        .iter()
        .map(|edge| (edge.id, *edge))
        .collect::<HashMap<_, _>>();
    let selected_sites = selected_edges
        .iter()
        .filter_map(|edge_id| edge_map.get(edge_id))
        .flat_map(|edge| edge.sites)
        .filter(|site_id| {
            sites
                .get(site_id)
                .is_some_and(|site| site.surface_kind.is_land_owned())
        })
        .collect::<HashSet<_>>();
    if selected_sites.is_empty() {
        return HashSet::new();
    }

    let mut extensions = HashSet::new();

    for edge in edges {
        if selected_edges.contains(&edge.id) || !edge.guide.is_coast {
            continue;
        }

        for site_id in edge.sites {
            let Some(site) = sites.get(&site_id) else {
                continue;
            };
            if site.surface_kind.is_land_owned()
                && selected_sites.contains(&site_id)
                && site.distance_to_coast_blocks <= config.coast_width_blocks * 1.6
            {
                extensions.insert(edge.id);
                break;
            }
        }
    }

    extensions
}

fn best_corner_connected_downstream_edge(
    current_index: usize,
    edges: &[MacroEdge],
    sites: &HashMap<VoronoiSiteId, MacroSite>,
    corner_edges: &HashMap<VoronoiCornerId, Vec<usize>>,
    visited: &HashSet<VoronoiEdgeId>,
) -> Option<usize> {
    let current = edges[current_index];
    let current_drainage = river_corridor_edge_drainage(current, sites);
    let current_coast_distance = river_corridor_edge_coast_distance(current, sites);
    let mut best = None::<(f32, usize)>;

    for corner in current.corners {
        let Some(neighbor_indices) = corner_edges.get(&corner) else {
            continue;
        };
        for &next_index in neighbor_indices {
            if next_index == current_index || visited.contains(&edges[next_index].id) {
                continue;
            }
            let next = edges[next_index];
            if !is_river_corridor_edge_candidate(next, sites) {
                continue;
            }

            let next_drainage = river_corridor_edge_drainage(next, sites);
            let descent = current_drainage - next_drainage;
            let coast_progress =
                current_coast_distance - river_corridor_edge_coast_distance(next, sites);
            let soft_downhill = descent >= -0.012 && coast_progress > 0.0;
            if descent < 0.006 && !soft_downhill {
                continue;
            }

            let score = (descent.max(0.0) * 1.72
                + coast_progress.max(0.0) * 0.0028
                + next.guide.river_potential * 0.32
                + river_corridor_edge_basinness(next, sites) * 0.18
                + river_corridor_edge_coastness(next, sites) * 0.08
                - next.guide.ridgeness * 0.12)
                .clamp(0.0, 1.0);
            if best.is_none_or(|(best_score, best_index)| {
                score > best_score || (score == best_score && next.id.0 < edges[best_index].id.0)
            }) {
                best = Some((score, next_index));
            }
        }
    }

    best.and_then(|(score, index)| (score >= 0.25).then_some(index))
}

fn is_river_corridor_edge_candidate(
    edge: MacroEdge,
    sites: &HashMap<VoronoiSiteId, MacroSite>,
) -> bool {
    if edge.guide.is_ridge_candidate {
        return false;
    }

    let Some(a) = sites.get(&edge.sites[0]).copied() else {
        return false;
    };
    let Some(b) = sites.get(&edge.sites[1]).copied() else {
        return false;
    };

    let a_land = a.surface_kind.is_land_owned();
    let b_land = b.surface_kind.is_land_owned();
    if edge.guide.is_coast {
        return a_land != b_land;
    }

    a_land && b_land && (a.signed_macro_elevation + b.signed_macro_elevation) * 0.5 > 0.025
}

fn river_corridor_edge_source_score(
    edge: MacroEdge,
    sites: &HashMap<VoronoiSiteId, MacroSite>,
    config: MacroMapConfig,
) -> Option<f32> {
    let a = sites.get(&edge.sites[0]).copied()?;
    let b = sites.get(&edge.sites[1]).copied()?;
    let elevation = (a.signed_macro_elevation + b.signed_macro_elevation) * 0.5;
    let basinness = (a.basinness + b.basinness) * 0.5;
    let inlandness = (river_corridor_edge_coast_distance(edge, sites)
        / (config.coast_width_blocks * 4.0))
        .clamp(0.0, 1.0);

    Some(
        (elevation * 0.34
            + basinness * 0.24
            + river_corridor_edge_mountainness(edge, sites) * 0.16
            + inlandness * 0.16
            + edge.guide.river_potential * 0.10)
            .max(0.0),
    )
}

fn river_corridor_edge_drainage(edge: MacroEdge, sites: &HashMap<VoronoiSiteId, MacroSite>) -> f32 {
    let Some(a) = sites.get(&edge.sites[0]).copied() else {
        return 0.0;
    };
    let Some(b) = sites.get(&edge.sites[1]).copied() else {
        return 0.0;
    };

    (drainage_elevation(a) + drainage_elevation(b)) * 0.5
}

fn river_corridor_edge_coast_distance(
    edge: MacroEdge,
    sites: &HashMap<VoronoiSiteId, MacroSite>,
) -> f32 {
    let Some(a) = sites.get(&edge.sites[0]).copied() else {
        return 0.0;
    };
    let Some(b) = sites.get(&edge.sites[1]).copied() else {
        return 0.0;
    };

    (a.distance_to_coast_blocks + b.distance_to_coast_blocks) * 0.5
}

fn river_corridor_edge_coastness(
    edge: MacroEdge,
    sites: &HashMap<VoronoiSiteId, MacroSite>,
) -> f32 {
    let Some(a) = sites.get(&edge.sites[0]).copied() else {
        return 0.0;
    };
    let Some(b) = sites.get(&edge.sites[1]).copied() else {
        return 0.0;
    };

    (a.coastness + b.coastness) * 0.5
}

fn river_corridor_edge_basinness(
    edge: MacroEdge,
    sites: &HashMap<VoronoiSiteId, MacroSite>,
) -> f32 {
    let Some(a) = sites.get(&edge.sites[0]).copied() else {
        return 0.0;
    };
    let Some(b) = sites.get(&edge.sites[1]).copied() else {
        return 0.0;
    };

    (a.basinness + b.basinness) * 0.5
}

fn river_corridor_edge_mountainness(
    edge: MacroEdge,
    sites: &HashMap<VoronoiSiteId, MacroSite>,
) -> f32 {
    let Some(a) = sites.get(&edge.sites[0]).copied() else {
        return 0.0;
    };
    let Some(b) = sites.get(&edge.sites[1]).copied() else {
        return 0.0;
    };

    (a.mountainness + b.mountainness) * 0.5
}

fn deterministic_corridor_edge_bias(edge_id: VoronoiEdgeId, config: MacroMapConfig) -> f32 {
    unit_f32(splitmix64(
        splitmix64(edge_id.0 ^ 0xbacf_17e5_d0c0_51de)
            ^ splitmix64(config.seed)
            ^ u64::from(config.generator_version),
    ))
}

#[derive(Debug, Clone, Copy)]
struct RiverCorridorStep {
    edge_id: VoronoiEdgeId,
    site_id: VoronoiSiteId,
    edge_potential: f32,
}

#[derive(Debug, Clone, Copy)]
struct RankedRiverCorridorStep {
    edge_id: VoronoiEdgeId,
    site_id: VoronoiSiteId,
    score: f32,
}

fn river_corridor_adjacency(
    edges: &[MacroEdge],
    sites: &HashMap<VoronoiSiteId, MacroSite>,
) -> HashMap<VoronoiSiteId, Vec<RiverCorridorStep>> {
    let mut adjacency = HashMap::<VoronoiSiteId, Vec<RiverCorridorStep>>::new();

    for edge in edges {
        if edge.guide.is_ridge_candidate {
            continue;
        }

        let Some(a) = sites.get(&edge.sites[0]).copied() else {
            continue;
        };
        let Some(b) = sites.get(&edge.sites[1]).copied() else {
            continue;
        };
        let a_land = a.surface_kind.is_land_owned();
        let b_land = b.surface_kind.is_land_owned();
        if !a_land && !b_land {
            continue;
        }

        if a_land {
            adjacency.entry(a.id).or_default().push(RiverCorridorStep {
                edge_id: edge.id,
                site_id: b.id,
                edge_potential: edge.guide.river_potential,
            });
        }
        if b_land {
            adjacency.entry(b.id).or_default().push(RiverCorridorStep {
                edge_id: edge.id,
                site_id: a.id,
                edge_potential: edge.guide.river_potential,
            });
        }
    }

    adjacency.par_iter_mut().for_each(|(_, steps)| {
        steps.sort_by_key(|step| (step.site_id.0, step.edge_id.0));
        steps.dedup_by_key(|step| (step.site_id, step.edge_id));
    });

    adjacency
}

fn is_river_corridor_source(site: &MacroSite, config: MacroMapConfig) -> bool {
    site.surface_kind.is_land_owned()
        && site.signed_macro_elevation >= 0.14
        && site.basinness >= 0.34
        && site.coastness <= 0.58
        && site.distance_to_coast_blocks >= config.coast_width_blocks * 0.45
}

fn river_corridor_source_score(site: &MacroSite, config: MacroMapConfig) -> f32 {
    let inlandness =
        (site.distance_to_coast_blocks / (config.coast_width_blocks * 4.0)).clamp(0.0, 1.0);
    (site.signed_macro_elevation * 0.34
        + site.basinness * 0.24
        + site.mountainness * 0.18
        + inlandness * 0.14
        + (1.0 - site.coastness) * 0.10)
        .max(0.0)
}

fn best_downstream_corridor_step(
    current: MacroSite,
    adjacency: &HashMap<VoronoiSiteId, Vec<RiverCorridorStep>>,
    sites: &HashMap<VoronoiSiteId, MacroSite>,
    visited_sites: &HashSet<VoronoiSiteId>,
    minimum_edge_score: f32,
    config: MacroMapConfig,
) -> Option<RankedRiverCorridorStep> {
    let current_drainage = drainage_elevation(current);
    let current_coast_distance = current.distance_to_coast_blocks;

    adjacency
        .get(&current.id)?
        .iter()
        .filter_map(|step| {
            if visited_sites.contains(&step.site_id) {
                return None;
            }

            let next = sites.get(&step.site_id).copied()?;
            let next_drainage = drainage_elevation(next);
            let descent = current_drainage - next_drainage;
            let coast_progress = (current_coast_distance - next.distance_to_coast_blocks)
                / config.coast_width_blocks;
            let ocean_basin_progress = (current.distance_to_ocean_basin_blocks
                - next.distance_to_ocean_basin_blocks)
                / config.super_cell_size_blocks as f32;
            let outlet_progress = coast_progress.max(ocean_basin_progress);
            let outlet_step = !next.surface_kind.is_land_owned();
            let soft_downhill = descent >= -0.015 && coast_progress > 0.10;
            let soft_outlet_step = descent >= -0.010 && outlet_progress > 0.045;
            if descent < 0.010 && !soft_downhill && !soft_outlet_step && !outlet_step {
                return None;
            }

            let score = (descent.max(0.0) * 1.85
                + outlet_progress.max(0.0) * 0.30
                + next.basinness * 0.18
                + step.edge_potential * 0.34
                + next.coastness * 0.08
                + if outlet_step { 0.28 } else { 0.0 }
                - next.ridgeness * 0.16)
                .clamp(0.0, 1.0);
            if score < minimum_edge_score {
                return None;
            }

            Some(RankedRiverCorridorStep {
                edge_id: step.edge_id,
                site_id: step.site_id,
                score,
            })
        })
        .max_by(|a, b| {
            a.score
                .total_cmp(&b.score)
                .then_with(|| b.edge_id.0.cmp(&a.edge_id.0))
        })
}

fn drainage_elevation(site: MacroSite) -> f32 {
    site.signed_macro_elevation - site.basinness * 0.09 - site.coastness * 0.06
}

fn empty_edge_guide() -> MacroEdgeGuide {
    MacroEdgeGuide {
        is_coast: false,
        is_ridge_candidate: false,
        is_river_candidate: false,
        is_fault_candidate: false,
        coastness: 0.0,
        ridgeness: 0.0,
        river_potential: 0.0,
    }
}

fn nearest_macro_core(
    position: WorldPlanePoint,
    kind: MacroCoreKind,
    config: MacroMapConfig,
) -> MacroCore {
    let (cell_x, cell_z) = super_cell_coord(position, config.super_cell_size_blocks);
    let mut best = None::<(f32, MacroCore)>;

    for z in (cell_z - CORE_SEARCH_RADIUS_CELLS)..=(cell_z + CORE_SEARCH_RADIUS_CELLS) {
        for x in (cell_x - CORE_SEARCH_RADIUS_CELLS)..=(cell_x + CORE_SEARCH_RADIUS_CELLS) {
            if macro_core_kind(x, z, config) != kind {
                continue;
            }

            let core = macro_core(x, z, kind, config);
            let distance_squared = distance_squared(position, core.position);
            if best.is_none_or(|(best_distance, _)| distance_squared < best_distance) {
                best = Some((distance_squared, core));
            }
        }
    }

    best.unwrap_or_else(|| {
        let (fallback_x, fallback_z) = nearest_forced_core_cell(cell_x, cell_z, kind, config);
        (0.0, macro_core(fallback_x, fallback_z, kind, config))
    })
    .1
}

fn macro_core(x: i32, z: i32, kind: MacroCoreKind, config: MacroMapConfig) -> MacroCore {
    let namespace = match kind {
        MacroCoreKind::Continent => HASH_CONTINENT_FIELD,
        MacroCoreKind::Ocean => HASH_OCEAN_FIELD,
    };
    let hash = macro_hash(config, namespace, x, z, 0);
    let size = config.super_cell_size_blocks as f32;
    let jitter_radius = size * 0.34;
    let jitter_x = (unit_f32(hash) * 2.0 - 1.0) * jitter_radius;
    let jitter_z = (unit_f32(splitmix64(hash ^ 0x9e37_79b9_7f4a_7c15)) * 2.0 - 1.0) * jitter_radius;
    let id_namespace = match kind {
        MacroCoreKind::Continent => 0x0c01_71ab_1e00_0001,
        MacroCoreKind::Ocean => 0x0cea_71ab_1e00_0002,
    };

    MacroCore {
        id: macro_hash(config, id_namespace, x, z, 1),
        position: WorldPlanePoint::new(
            x as f32 * size + size * 0.5 + jitter_x,
            z as f32 * size + size * 0.5 + jitter_z,
        ),
    }
}

fn macro_core_kind(x: i32, z: i32, config: MacroMapConfig) -> MacroCoreKind {
    let size = config.super_cell_size_blocks as f32;
    let position = WorldPlanePoint::new(x as f32 * size + size * 0.5, z as f32 * size + size * 0.5);
    if continent_field(position, config) >= -0.08 {
        MacroCoreKind::Continent
    } else {
        MacroCoreKind::Ocean
    }
}

fn nearest_forced_core_cell(
    cell_x: i32,
    cell_z: i32,
    kind: MacroCoreKind,
    config: MacroMapConfig,
) -> (i32, i32) {
    let mut best = None::<(f32, i32, i32)>;

    for z in (cell_z - CORE_SEARCH_RADIUS_CELLS * 2)..=(cell_z + CORE_SEARCH_RADIUS_CELLS * 2) {
        for x in (cell_x - CORE_SEARCH_RADIUS_CELLS * 2)..=(cell_x + CORE_SEARCH_RADIUS_CELLS * 2) {
            let hash = macro_hash(config, HASH_CONTINENT_FIELD, x, z, kind as i32);
            let prefers_kind = (hash & 1) == matches!(kind, MacroCoreKind::Continent) as u64;
            if !prefers_kind {
                continue;
            }

            let dx = (x - cell_x) as f32;
            let dz = (z - cell_z) as f32;
            let distance_squared = dx.mul_add(dx, dz * dz);
            if best.is_none_or(|(best_distance, _, _)| distance_squared < best_distance) {
                best = Some((distance_squared, x, z));
            }
        }
    }

    best.map(|(_, x, z)| (x, z)).unwrap_or((cell_x, cell_z))
}

fn warped_macro_position(position: WorldPlanePoint, config: MacroMapConfig) -> WorldPlanePoint {
    let scale = config.super_cell_size_blocks as f32 * 1.75;
    let warp_x = fbm_signed(position, scale, HASH_COAST_WARP_FIELD, config);
    let warp_z = fbm_signed(
        WorldPlanePoint::new(position.z + 1013.0, position.x - 719.0),
        scale,
        HASH_COAST_WARP_FIELD ^ 0xa9d8_4c31_7f07_d351,
        config,
    );
    let amount = config.super_cell_size_blocks as f32 * 0.34;

    WorldPlanePoint::new(position.x + warp_x * amount, position.z + warp_z * amount)
}

fn continent_field(position: WorldPlanePoint, config: MacroMapConfig) -> f32 {
    let size = config.super_cell_size_blocks as f32;
    let broad = fbm_signed(position, size * 3.8, HASH_CONTINENT_FIELD, config);
    let lobe = fbm_signed(
        position,
        size * 1.55,
        HASH_CONTINENT_FIELD ^ 0x91e3_57d2_c1aa_09b5,
        config,
    );
    let tendril = fbm_signed(
        position,
        size * 0.78,
        HASH_CONTINENT_FIELD ^ 0x24b6_b08d_34ac_4491,
        config,
    );

    clamp_signed(broad * 0.62 + lobe * 0.28 + tendril * 0.10 + 0.14)
}

fn island_field(position: WorldPlanePoint, large_landmass: f32, config: MacroMapConfig) -> f32 {
    let size = config.super_cell_size_blocks as f32;
    let archipelago = fbm_unit(position, size * 0.72, HASH_ISLAND_FIELD, config);
    let small_island = value_noise_unit(
        position,
        size * 0.28,
        HASH_ISLAND_FIELD ^ 0x5e7a_1d5a_1204_d11d,
        config,
    );
    let offshore_mask = smoothstep(-0.74, -0.18, -large_landmass);
    let island_peak =
        smoothstep(0.64, 0.90, archipelago) * 0.78 + smoothstep(0.82, 0.98, small_island) * 0.50;

    island_peak * offshore_mask
}

fn deterministic_ridge_wave(position: WorldPlanePoint, config: MacroMapConfig) -> f32 {
    let cell_size = (config.super_cell_size_blocks as f32 * 0.5).max(1.0);
    let x = (position.x / cell_size).floor() as i32;
    let z = (position.z / cell_size).floor() as i32;
    let hash = macro_hash(config, HASH_MOUNTAIN_FIELD, x, z, 0);
    let phase = unit_f32(hash) * std::f32::consts::TAU;
    let direction = unit_f32(splitmix64(hash ^ 0x517c_c1b7_2722_0a95)) * std::f32::consts::TAU;
    let wave_x = direction.cos();
    let wave_z = direction.sin();
    let projected = (position.x * wave_x + position.z * wave_z) / cell_size + phase;
    let line_wave = 1.0 - projected.sin().abs();
    let patch_noise = unit_f32(splitmix64(hash ^ 0xd1b5_4a32_d192_ed03));
    let broad_highland = fbm_unit(
        position,
        config.super_cell_size_blocks as f32 * 0.92,
        HASH_MOUNTAIN_FIELD ^ 0x7065_c3d0_21af_99c7,
        config,
    );
    let chain = 1.0 - (line_wave - broad_highland * 0.30).abs().clamp(0.0, 1.0);

    (chain * 0.58 + line_wave * 0.20 + patch_noise * 0.22).clamp(0.0, 1.0)
}

fn fbm_signed(
    position: WorldPlanePoint,
    scale_blocks: f32,
    namespace: u64,
    config: MacroMapConfig,
) -> f32 {
    fbm_unit(position, scale_blocks, namespace, config) * 2.0 - 1.0
}

fn fbm_unit(
    position: WorldPlanePoint,
    scale_blocks: f32,
    namespace: u64,
    config: MacroMapConfig,
) -> f32 {
    let first = value_noise_unit(position, scale_blocks, namespace, config);
    let second = value_noise_unit(
        WorldPlanePoint::new(position.x + 349.0, position.z - 577.0),
        scale_blocks * 0.52,
        namespace ^ 0x6d2b_79f5_aa73_19c9,
        config,
    );
    let third = value_noise_unit(
        WorldPlanePoint::new(position.x - 911.0, position.z + 233.0),
        scale_blocks * 0.27,
        namespace ^ 0xf17b_1a2c_45f1_08ea,
        config,
    );

    (first * 0.56 + second * 0.30 + third * 0.14).clamp(0.0, 1.0)
}

fn value_noise_unit(
    position: WorldPlanePoint,
    scale_blocks: f32,
    namespace: u64,
    config: MacroMapConfig,
) -> f32 {
    let scale = scale_blocks.max(1.0);
    let x = position.x / scale;
    let z = position.z / scale;
    let x0 = x.floor() as i32;
    let z0 = z.floor() as i32;
    let tx = smooth_unit(x - x0 as f32);
    let tz = smooth_unit(z - z0 as f32);
    let a = lattice_unit(config, namespace, x0, z0);
    let b = lattice_unit(config, namespace, x0 + 1, z0);
    let c = lattice_unit(config, namespace, x0, z0 + 1);
    let d = lattice_unit(config, namespace, x0 + 1, z0 + 1);
    let top = lerp(a, b, tx);
    let bottom = lerp(c, d, tx);

    lerp(top, bottom, tz).clamp(0.0, 1.0)
}

fn lattice_unit(config: MacroMapConfig, namespace: u64, x: i32, z: i32) -> f32 {
    unit_f32(macro_hash(config, namespace, x, z, 0))
}

fn smooth_unit(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, amount: f32) -> f32 {
    a + (b - a) * amount
}

fn super_cell_coord(position: WorldPlanePoint, super_cell_size_blocks: i32) -> (i32, i32) {
    let size = super_cell_size_blocks as f32;
    (
        (position.x / size).floor() as i32,
        (position.z / size).floor() as i32,
    )
}

fn distance(a: WorldPlanePoint, b: WorldPlanePoint) -> f32 {
    distance_squared(a, b).sqrt()
}

fn distance_squared(a: WorldPlanePoint, b: WorldPlanePoint) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    dx.mul_add(dx, dz * dz)
}

fn macro_hash(config: MacroMapConfig, namespace: u64, x: i32, z: i32, extra: i32) -> u64 {
    let mut state = splitmix64(config.seed ^ namespace);
    state = splitmix64(state ^ u64::from(config.generator_version));
    state = splitmix64(state ^ u64::from(config.super_cell_size_blocks as u32));
    state = splitmix64(state ^ u64::from(zigzag_i32(x)));
    state = splitmix64(state ^ u64::from(zigzag_i32(z)));
    splitmix64(state ^ u64::from(zigzag_i32(extra)))
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn unit_f32(value: u64) -> f32 {
    const SCALE: f64 = 1.0 / ((1u64 << 53) as f64);
    (((value >> 11) as f64) * SCALE) as f32
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

fn zigzag_i32(value: i32) -> u32 {
    ((value << 1) ^ (value >> 31)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generation::graph::{
        DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS, VoronoiGraphConfig,
        VoronoiGraphPatchRequest, generate_voronoi_graph_patch,
    };

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
        let patch = generate_voronoi_graph_patch(test_request(7, 0, 0));
        let map = generate_macro_map(&patch, test_macro_config(7));

        assert!(
            map.sites
                .iter()
                .any(|site| site.surface_kind.is_land_owned())
        );
        assert!(
            map.sites
                .iter()
                .any(|site| site.surface_kind.is_ocean_owned())
        );
        assert!(
            map.sites
                .iter()
                .any(|site| site.signed_macro_elevation > 0.0)
        );
        assert!(
            map.sites
                .iter()
                .any(|site| site.signed_macro_elevation < 0.0)
        );
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
        let patch = generate_voronoi_graph_patch(test_request(12, 0, 0));
        let map = generate_macro_map(&patch, test_macro_config(12));
        let site_map = macro_sites_by_id(&map);
        let coast_edges = map.coast_edges().collect::<Vec<_>>();

        assert!(!coast_edges.is_empty());
        for edge in coast_edges {
            let a = site_map[&edge.sites[0]];
            let b = site_map[&edge.sites[1]];
            assert_ne!(
                a.surface_kind.is_land_owned(),
                b.surface_kind.is_land_owned()
            );
        }
    }

    #[test]
    fn ridge_and_river_candidates_are_not_empty() {
        let patch = generate_voronoi_graph_patch(test_request(91, 0, 0));
        let mut config = test_macro_config(91);
        config.ridge_candidate_threshold = 0.46;
        config.river_candidate_threshold = 0.46;

        let map = generate_macro_map(&patch, config);

        assert!(map.ridge_candidate_edges().next().is_some());
        assert!(map.river_candidate_edges().next().is_some());
    }

    #[test]
    fn river_candidates_form_pre_hydrology_downstream_corridors() {
        let patch = generate_voronoi_graph_patch(test_request(91, 0, 0));
        let mut config = test_macro_config(91);
        config.ridge_candidate_threshold = 0.46;
        config.river_candidate_threshold = 0.46;
        let map = generate_macro_map(&patch, config);
        let site_map = macro_sites_by_id(&map);
        let river_edges = map.river_candidate_edges().collect::<Vec<_>>();

        assert!(!river_edges.is_empty());
        for edge in &river_edges {
            let a = site_map[&edge.sites[0]];
            let b = site_map[&edge.sites[1]];

            if edge.guide.is_coast {
                assert_ne!(
                    a.surface_kind.is_land_owned(),
                    b.surface_kind.is_land_owned()
                );
            } else {
                assert!(a.surface_kind.is_land_owned());
                assert!(b.surface_kind.is_land_owned());
            }
            assert!(!edge.guide.is_ridge_candidate);
        }

        let components = river_candidate_components(&river_edges);
        let best = components
            .iter()
            .filter_map(|component| {
                let mut min_elevation = f32::INFINITY;
                let mut max_elevation = f32::NEG_INFINITY;
                let mut min_outlet_distance = f32::INFINITY;
                let mut max_outlet_distance = f32::NEG_INFINITY;

                for site_id in &component.site_ids {
                    let site = site_map[site_id];
                    min_elevation = min_elevation.min(site.signed_macro_elevation);
                    max_elevation = max_elevation.max(site.signed_macro_elevation);
                    let outlet_distance = site.distance_to_ocean_basin_blocks;
                    min_outlet_distance = min_outlet_distance.min(outlet_distance);
                    max_outlet_distance = max_outlet_distance.max(outlet_distance);
                }

                (component.edge_count >= 3).then_some((
                    component.edge_count,
                    max_elevation - min_elevation,
                    max_outlet_distance - min_outlet_distance,
                ))
            })
            .max_by(|a, b| a.0.cmp(&b.0));

        let Some((edge_count, elevation_drop, outlet_progress)) = best else {
            panic!("expected at least one connected river candidate corridor");
        };
        assert!(edge_count >= 3);
        assert!(
            elevation_drop >= 0.05,
            "corridor should span high-to-low macro elevation; drop={elevation_drop}"
        );
        assert!(
            outlet_progress >= config.coast_width_blocks * 0.20,
            "corridor should make visible progress toward lower/coastal ground; progress={outlet_progress}"
        );
    }

    #[test]
    fn river_corridor_extension_can_mark_adjacent_coast_outlet_edge() {
        let config = test_macro_config(91);
        let land_head = MacroSite {
            id: VoronoiSiteId(1),
            owner_region: GraphRegionCoord::new(0, 0),
            position: WorldPlanePoint::new(0.0, 0.0),
            surface_kind: MacroSurfaceKind::Continent,
            continent: Some(MacroContinentId(1)),
            ocean_basin: None,
            signed_macro_elevation: 0.18,
            continentality: 0.24,
            coastness: 0.0,
            distance_to_coast_blocks: config.coast_width_blocks * 2.0,
            distance_to_continent_core_blocks: 128.0,
            distance_to_ocean_basin_blocks: 512.0,
            mountainness: 0.24,
            ridgeness: 0.12,
            basinness: 0.58,
        };
        let land_outlet = MacroSite {
            id: VoronoiSiteId(2),
            distance_to_coast_blocks: config.coast_width_blocks,
            coastness: 0.72,
            signed_macro_elevation: 0.05,
            ..land_head
        };
        let ocean = MacroSite {
            id: VoronoiSiteId(3),
            surface_kind: MacroSurfaceKind::CoastOcean,
            continent: None,
            ocean_basin: Some(MacroOceanBasinId(9)),
            signed_macro_elevation: -0.05,
            continentality: -0.18,
            coastness: 0.82,
            distance_to_coast_blocks: config.coast_width_blocks * 0.5,
            distance_to_ocean_basin_blocks: 128.0,
            ..land_head
        };
        let sites = HashMap::from([
            (land_head.id, land_head),
            (land_outlet.id, land_outlet),
            (ocean.id, ocean),
        ]);
        let river_edge_id = VoronoiEdgeId(10);
        let coast_edge_id = VoronoiEdgeId(11);
        let edges = [
            MacroEdge {
                id: river_edge_id,
                sites: [land_head.id, land_outlet.id],
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                guide: MacroEdgeGuide {
                    is_coast: false,
                    is_ridge_candidate: false,
                    is_river_candidate: true,
                    is_fault_candidate: false,
                    coastness: 0.0,
                    ridgeness: 0.0,
                    river_potential: 0.72,
                },
            },
            MacroEdge {
                id: coast_edge_id,
                sites: [land_outlet.id, ocean.id],
                corners: [VoronoiCornerId(2), VoronoiCornerId(3)],
                guide: MacroEdgeGuide {
                    is_coast: true,
                    is_ridge_candidate: false,
                    is_river_candidate: false,
                    is_fault_candidate: false,
                    coastness: 1.0,
                    ridgeness: 0.0,
                    river_potential: 0.0,
                },
            },
        ];
        let selected = HashSet::from([river_edge_id]);

        let extensions = river_corridor_outlet_extensions(&edges, &sites, &selected, config);

        assert!(extensions.contains(&coast_edge_id));
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

    fn test_macro_config(seed: u64) -> MacroMapConfig {
        MacroMapConfig {
            seed,
            generator_version: 3,
            super_cell_size_blocks: 768,
            sea_level: 0.0,
            coast_width_blocks: 192.0,
            ridge_candidate_threshold: DEFAULT_MACRO_RIDGE_CANDIDATE_THRESHOLD,
            river_candidate_threshold: DEFAULT_MACRO_RIVER_CANDIDATE_THRESHOLD,
            land_bias: DEFAULT_MACRO_LAND_BIAS,
            island_strength: DEFAULT_MACRO_ISLAND_STRENGTH,
        }
    }

    fn macro_sites_by_id(map: &GraphMacroMap) -> HashMap<VoronoiSiteId, MacroSite> {
        map.sites.iter().map(|site| (site.id, *site)).collect()
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
    struct RiverCandidateComponent {
        site_ids: HashSet<VoronoiSiteId>,
        edge_count: usize,
    }

    fn river_candidate_components(edges: &[&MacroEdge]) -> Vec<RiverCandidateComponent> {
        let mut adjacency = HashMap::<VoronoiSiteId, Vec<(VoronoiSiteId, VoronoiEdgeId)>>::new();
        for edge in edges {
            adjacency
                .entry(edge.sites[0])
                .or_default()
                .push((edge.sites[1], edge.id));
            adjacency
                .entry(edge.sites[1])
                .or_default()
                .push((edge.sites[0], edge.id));
        }

        let mut visited_sites = HashSet::new();
        let mut components = Vec::new();
        let mut starts = adjacency.keys().copied().collect::<Vec<_>>();
        starts.sort_by_key(|site| site.0);

        for start in starts {
            if visited_sites.contains(&start) {
                continue;
            }

            let mut stack = vec![start];
            let mut site_ids = HashSet::new();
            let mut edge_ids = HashSet::new();
            visited_sites.insert(start);

            while let Some(site_id) = stack.pop() {
                site_ids.insert(site_id);
                if let Some(neighbors) = adjacency.get(&site_id) {
                    for &(neighbor_id, edge_id) in neighbors {
                        edge_ids.insert(edge_id);
                        if visited_sites.insert(neighbor_id) {
                            stack.push(neighbor_id);
                        }
                    }
                }
            }

            components.push(RiverCandidateComponent {
                site_ids,
                edge_count: edge_ids.len(),
            });
        }

        components
    }
}
