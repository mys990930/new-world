use rayon::prelude::*;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, VecDeque};

use super::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiGraphPatch, WorldPlanePoint};
use super::macro_map::{GraphMacroMap, MacroCorner, MacroEdge, MacroSurfaceKind};

pub const DEFAULT_RIVER_FLOW_THRESHOLD: f32 = 10.0;
pub const DEFAULT_HEADWATER_ELEVATION: f32 = 0.10;
pub const DEFAULT_LAKE_RIVER_FLOW_THRESHOLD_MULTIPLIER: f32 = 5.0;
pub const DEFAULT_LAKE_DISCHARGE_CAP_PER_AREA: f32 = 0.35;
pub const DEFAULT_LAKE_DISCHARGE_CAP_FLOOR: f32 = 4.0;
pub const DEFAULT_LAKE_DISCHARGE_CAP_CEILING: f32 = 32.0;
pub const DEFAULT_LAKE_DISCHARGE_RANGE_PER_AREA: f32 = 0.18;
pub const DEFAULT_LAKE_AREA_UNITS_PER_CHAIN: f32 = 24.0;
pub const DEFAULT_LAKE_MAX_INCOMING_CHAINS: usize = 3;
pub const DEFAULT_LAKE_MAX_OUTLETS_PER_COMPONENT: usize = 2;
pub const DEFAULT_LAKE_INLET_OUTLET_MIN_EDGE_HOPS: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HydrologyConfig {
    pub river_flow_threshold: f32,
    pub headwater_elevation: f32,
    pub lake_river_flow_threshold_multiplier: f32,
    pub lake_discharge_cap_per_area: f32,
    pub lake_discharge_cap_floor: f32,
    pub lake_discharge_cap_ceiling: f32,
    pub lake_discharge_range_per_area: f32,
    pub lake_area_units_per_chain: f32,
    pub lake_max_incoming_chains: usize,
    pub lake_max_outlets_per_component: usize,
    pub lake_inlet_outlet_min_edge_hops: u32,
}

impl Default for HydrologyConfig {
    fn default() -> Self {
        Self {
            river_flow_threshold: DEFAULT_RIVER_FLOW_THRESHOLD,
            headwater_elevation: DEFAULT_HEADWATER_ELEVATION,
            lake_river_flow_threshold_multiplier: DEFAULT_LAKE_RIVER_FLOW_THRESHOLD_MULTIPLIER,
            lake_discharge_cap_per_area: DEFAULT_LAKE_DISCHARGE_CAP_PER_AREA,
            lake_discharge_cap_floor: DEFAULT_LAKE_DISCHARGE_CAP_FLOOR,
            lake_discharge_cap_ceiling: DEFAULT_LAKE_DISCHARGE_CAP_CEILING,
            lake_discharge_range_per_area: DEFAULT_LAKE_DISCHARGE_RANGE_PER_AREA,
            lake_area_units_per_chain: DEFAULT_LAKE_AREA_UNITS_PER_CHAIN,
            lake_max_incoming_chains: DEFAULT_LAKE_MAX_INCOMING_CHAINS,
            lake_max_outlets_per_component: DEFAULT_LAKE_MAX_OUTLETS_PER_COMPONENT,
            lake_inlet_outlet_min_edge_hops: DEFAULT_LAKE_INLET_OUTLET_MIN_EDGE_HOPS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WatershedId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphDrainageNodeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphRiverSegmentId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphDrainageNodeKind {
    Source,
    Confluence,
    LakeInlet,
    LakeOutlet,
    Lake,
    Sink,
    CoastOutlet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphHydrologyRole {
    None,
    Divide,
    Headwater,
    Tributary,
    Trunk,
    Floodplain,
    CoastOutlet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphLocalMinimumResolution {
    None,
    OceanOutlet,
    OutletCarve,
    Lake,
    Sink,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphHydrologyCorner {
    pub id: VoronoiCornerId,
    pub position: WorldPlanePoint,
    pub elevation: f32,
    pub downstream: Option<VoronoiCornerId>,
    pub downstream_edge: Option<VoronoiEdgeId>,
    pub watershed: WatershedId,
    pub flow_accumulation: f32,
    pub is_local_minimum: bool,
    pub resolution: GraphLocalMinimumResolution,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphDrainageNode {
    pub id: GraphDrainageNodeId,
    pub kind: GraphDrainageNodeKind,
    pub corner: VoronoiCornerId,
    pub position: WorldPlanePoint,
    pub watershed: WatershedId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphRiverSegment {
    pub id: GraphRiverSegmentId,
    pub edge: VoronoiEdgeId,
    pub from: GraphDrainageNodeId,
    pub to: GraphDrainageNodeId,
    pub watershed: WatershedId,
    pub role: GraphHydrologyRole,
    pub raw_flow_accumulation: f32,
    pub flow_accumulation: f32,
    pub downstream_progress: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GraphHydrologyTopologyStats {
    pub lake_inlet_count: usize,
    pub lake_outlet_count: usize,
    pub disconnected_lake_inlet_count: usize,
    pub disconnected_lake_outlet_count: usize,
    pub selected_lake_edge_segment_count: usize,
    pub invalid_lake_contact_count: usize,
    pub invalid_river_intersection_count: usize,
    pub ambiguous_shared_corner_count: usize,
    pub duplicate_trunk_pruned_count: usize,
    pub repeated_lake_contact_pruned_count: usize,
    pub unclassified_lake_connected_flow_count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct GraphHydrologyGraph {
    pub corners: Vec<GraphHydrologyCorner>,
    pub nodes: Vec<GraphDrainageNode>,
    pub segments: Vec<GraphRiverSegment>,
    pub topology_stats: GraphHydrologyTopologyStats,
}

impl GraphHydrologyGraph {
    pub fn is_empty(&self) -> bool {
        self.corners.is_empty() && self.nodes.is_empty() && self.segments.is_empty()
    }

    pub fn corner(&self, id: VoronoiCornerId) -> Option<&GraphHydrologyCorner> {
        self.corners.iter().find(|corner| corner.id == id)
    }

    pub fn segments_for_edge(
        &self,
        edge: VoronoiEdgeId,
    ) -> impl Iterator<Item = &GraphRiverSegment> {
        self.segments
            .iter()
            .filter(move |segment| segment.edge == edge)
    }
}

pub fn solve_hydrology(
    patch: &VoronoiGraphPatch,
    macro_map: &GraphMacroMap,
    config: HydrologyConfig,
) -> GraphHydrologyGraph {
    validate_hydrology_config(config);

    let corner_map = macro_map
        .corners
        .iter()
        .map(|corner| (corner.id, *corner))
        .collect::<HashMap<_, _>>();
    let edge_map = macro_map
        .edges
        .iter()
        .map(|edge| (edge.id, *edge))
        .collect::<HashMap<_, _>>();
    let corner_indices = patch
        .corners
        .iter()
        .enumerate()
        .map(|(index, corner)| (corner.id, index))
        .collect::<HashMap<_, _>>();
    let adjacency = corner_adjacency(patch, &corner_indices);
    let terminals = patch
        .corners
        .iter()
        .map(|corner| {
            corner_map
                .get(&corner.id)
                .copied()
                .is_some_and(|macro_corner| is_terminal_outlet(macro_corner))
                || adjacency[corner_indices[&corner.id]]
                    .iter()
                    .any(|neighbor| {
                        edge_map
                            .get(&neighbor.edge)
                            .is_some_and(|edge| edge.guide.is_coast)
                    })
        })
        .collect::<Vec<_>>();
    let lake_candidates = patch
        .corners
        .iter()
        .map(|corner| {
            corner_map.get(&corner.id).copied().is_some_and(|corner| {
                matches!(
                    corner.surface_kind,
                    MacroSurfaceKind::LakeCandidate | MacroSurfaceKind::WetlandCandidate
                )
            })
        })
        .collect::<Vec<_>>();
    let elevations = patch
        .corners
        .par_iter()
        .map(|corner| {
            corner_map
                .get(&corner.id)
                .map(|macro_corner| macro_corner.signed_macro_elevation)
                .unwrap_or(corner.elevation)
        })
        .collect::<Vec<_>>();

    let (mut downstream, mut downstream_edges, local_minima, mut resolutions) =
        resolve_downstream(&elevations, &terminals, &lake_candidates, &adjacency);
    let lake_topology = resolve_lake_contact_topology(
        &mut downstream,
        &mut downstream_edges,
        &mut resolutions,
        &elevations,
        &lake_candidates,
        &adjacency,
        &edge_map,
        config,
    );
    let watersheds = resolve_watersheds(&downstream, &resolutions);
    let flow_accumulation = resolve_flow_accumulation(&downstream, &terminals, macro_map, patch);
    let terminal_indices = resolve_terminal_indices(&downstream);
    let first_downstream_lakes = resolve_first_downstream_lakes(&downstream, &lake_candidates);
    let lake_policies =
        resolve_lake_terminal_policies(&terminal_indices, &lake_candidates, &resolutions, config);
    let lake_inlet_policies = resolve_lake_inlet_policies(&lake_candidates, &adjacency, config);
    let selected_rivers = select_river_paths(
        &downstream,
        &downstream_edges,
        &flow_accumulation,
        &elevations,
        &terminals,
        &lake_candidates,
        &terminal_indices,
        &first_downstream_lakes,
        &lake_policies,
        &lake_inlet_policies,
        &lake_topology,
        &edge_map,
        config,
    );
    let selected = selected_rivers.selected;
    let node_kinds = resolve_node_kinds(
        &selected,
        &downstream,
        &flow_accumulation,
        &terminals,
        &resolutions,
        &lake_topology,
        &lake_inlet_policies,
        config,
    );
    let selected_flow_accumulation = resolve_selected_flow_accumulation(
        &flow_accumulation,
        &downstream,
        &terminal_indices,
        &first_downstream_lakes,
        &lake_policies,
        &lake_inlet_policies,
        &lake_topology,
        &node_kinds,
        config,
    );
    let topology_stats = resolve_topology_stats(
        &selected,
        &downstream,
        &lake_candidates,
        &resolutions,
        &downstream_edges,
        &edge_map,
        &node_kinds,
        selected_rivers.duplicate_trunk_pruned_count,
        selected_rivers.repeated_lake_contact_pruned_count,
    );
    let nodes = build_nodes(patch, &watersheds, &node_kinds);
    let segments = build_segments(
        &selected,
        &downstream,
        &downstream_edges,
        &flow_accumulation,
        &selected_flow_accumulation,
        &elevations,
        &watersheds,
        patch,
    );
    let mut corners = patch
        .corners
        .par_iter()
        .enumerate()
        .map(|(index, corner)| GraphHydrologyCorner {
            id: corner.id,
            position: corner.position,
            elevation: elevations[index],
            downstream: downstream[index].map(|target| patch.corners[target].id),
            downstream_edge: downstream_edges[index],
            watershed: watersheds[index],
            flow_accumulation: flow_accumulation[index],
            is_local_minimum: local_minima[index],
            resolution: resolutions[index],
        })
        .collect::<Vec<_>>();

    corners.sort_by_key(|corner| corner.id.0);

    GraphHydrologyGraph {
        corners,
        nodes,
        segments,
        topology_stats,
    }
}

#[derive(Debug, Clone, Copy)]
struct CornerNeighbor {
    index: usize,
    edge: VoronoiEdgeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectedRiverPaths {
    selected: Vec<bool>,
    duplicate_trunk_pruned_count: usize,
    repeated_lake_contact_pruned_count: usize,
}

fn validate_hydrology_config(config: HydrologyConfig) {
    assert!(
        config.river_flow_threshold.is_finite() && config.river_flow_threshold > 0.0,
        "river_flow_threshold must be positive and finite"
    );
    assert!(
        config.headwater_elevation.is_finite(),
        "headwater_elevation must be finite"
    );
    assert!(
        config.lake_river_flow_threshold_multiplier.is_finite()
            && config.lake_river_flow_threshold_multiplier >= 1.0,
        "lake_river_flow_threshold_multiplier must be finite and >= 1.0"
    );
    assert!(
        config.lake_discharge_cap_per_area.is_finite() && config.lake_discharge_cap_per_area > 0.0,
        "lake_discharge_cap_per_area must be positive and finite"
    );
    assert!(
        config.lake_discharge_cap_floor.is_finite() && config.lake_discharge_cap_floor > 0.0,
        "lake_discharge_cap_floor must be positive and finite"
    );
    assert!(
        config.lake_discharge_cap_ceiling.is_finite()
            && config.lake_discharge_cap_ceiling >= config.lake_discharge_cap_floor,
        "lake_discharge_cap_ceiling must be finite and >= lake_discharge_cap_floor"
    );
    assert!(
        config.lake_discharge_range_per_area.is_finite()
            && config.lake_discharge_range_per_area >= 0.0,
        "lake_discharge_range_per_area must be finite and >= 0"
    );
    assert!(
        config.lake_area_units_per_chain.is_finite() && config.lake_area_units_per_chain > 0.0,
        "lake_area_units_per_chain must be positive and finite"
    );
    assert!(
        config.lake_max_incoming_chains > 0,
        "lake_max_incoming_chains must be > 0"
    );
    assert!(
        config.lake_max_outlets_per_component <= 2,
        "lake_max_outlets_per_component must be <= 2 for launch topology"
    );
    assert!(
        config.lake_inlet_outlet_min_edge_hops > 0,
        "lake_inlet_outlet_min_edge_hops must be > 0"
    );
}

fn corner_adjacency(
    patch: &VoronoiGraphPatch,
    corner_indices: &HashMap<VoronoiCornerId, usize>,
) -> Vec<Vec<CornerNeighbor>> {
    let mut adjacency = vec![Vec::new(); patch.corners.len()];

    for edge in &patch.edges {
        let Some(&a) = corner_indices.get(&edge.corners[0]) else {
            continue;
        };
        let Some(&b) = corner_indices.get(&edge.corners[1]) else {
            continue;
        };
        adjacency[a].push(CornerNeighbor {
            index: b,
            edge: edge.id,
        });
        adjacency[b].push(CornerNeighbor {
            index: a,
            edge: edge.id,
        });
    }

    adjacency.par_iter_mut().for_each(|neighbors| {
        neighbors.sort_by_key(|neighbor| (neighbor.index, neighbor.edge.0));
        neighbors.dedup_by_key(|neighbor| (neighbor.index, neighbor.edge.0));
    });

    adjacency
}

fn is_terminal_outlet(corner: MacroCorner) -> bool {
    corner.surface_kind.is_ocean_owned()
        || matches!(corner.surface_kind, MacroSurfaceKind::CoastOcean)
}

fn resolve_downstream(
    elevations: &[f32],
    terminals: &[bool],
    lake_candidates: &[bool],
    adjacency: &[Vec<CornerNeighbor>],
) -> (
    Vec<Option<usize>>,
    Vec<Option<VoronoiEdgeId>>,
    Vec<bool>,
    Vec<GraphLocalMinimumResolution>,
) {
    let mut downstream = vec![None; elevations.len()];
    let mut downstream_edges = vec![None; elevations.len()];
    let mut local_minima = vec![false; elevations.len()];
    let mut resolutions = vec![GraphLocalMinimumResolution::None; elevations.len()];

    for index in 0..elevations.len() {
        if terminals[index] {
            resolutions[index] = GraphLocalMinimumResolution::OceanOutlet;
            continue;
        }

        if let Some(neighbor) = steepest_lower_neighbor(index, elevations, terminals, adjacency) {
            downstream[index] = Some(neighbor.index);
            downstream_edges[index] = Some(neighbor.edge);
        } else {
            local_minima[index] = true;
        }
    }

    for index in 0..elevations.len() {
        if !local_minima[index] {
            continue;
        }

        if lake_candidates[index] {
            resolutions[index] = GraphLocalMinimumResolution::Lake;
        } else if let Some(path) = spill_path_to_outlet(index, elevations, terminals, adjacency) {
            for pair in path.windows(2) {
                let from = pair[0];
                let to = pair[1];
                if terminals[from] {
                    continue;
                }
                downstream[from] = Some(to);
                downstream_edges[from] = edge_between(from, to, adjacency);
                resolutions[from] = if from == index {
                    GraphLocalMinimumResolution::OutletCarve
                } else {
                    resolutions[from]
                };
            }
        } else {
            resolutions[index] = GraphLocalMinimumResolution::Sink;
        }
    }

    break_remaining_cycles(&mut downstream, &mut downstream_edges, &mut resolutions);

    (downstream, downstream_edges, local_minima, resolutions)
}

fn steepest_lower_neighbor(
    index: usize,
    elevations: &[f32],
    terminals: &[bool],
    adjacency: &[Vec<CornerNeighbor>],
) -> Option<CornerNeighbor> {
    let current = elevations[index];

    adjacency[index]
        .iter()
        .copied()
        .filter(|neighbor| {
            terminals[neighbor.index] || elevations[neighbor.index] < current - 0.001
        })
        .min_by(|left, right| {
            let left_elevation = if terminals[left.index] {
                -1.0
            } else {
                elevations[left.index]
            };
            let right_elevation = if terminals[right.index] {
                -1.0
            } else {
                elevations[right.index]
            };
            left_elevation
                .total_cmp(&right_elevation)
                .then_with(|| left.index.cmp(&right.index))
        })
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SpillState {
    index: usize,
    spill_elevation: f32,
    path_len: u32,
}

impl Eq for SpillState {}

impl Ord for SpillState {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .spill_elevation
            .total_cmp(&self.spill_elevation)
            .then_with(|| other.path_len.cmp(&self.path_len))
            .then_with(|| other.index.cmp(&self.index))
    }
}

impl PartialOrd for SpillState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn spill_path_to_outlet(
    start: usize,
    elevations: &[f32],
    terminals: &[bool],
    adjacency: &[Vec<CornerNeighbor>],
) -> Option<Vec<usize>> {
    let mut heap = BinaryHeap::new();
    let mut best = vec![f32::INFINITY; elevations.len()];
    let mut previous = vec![None; elevations.len()];

    best[start] = elevations[start];
    heap.push(SpillState {
        index: start,
        spill_elevation: elevations[start],
        path_len: 0,
    });

    while let Some(state) = heap.pop() {
        if state.spill_elevation > best[state.index] + f32::EPSILON {
            continue;
        }
        if state.index != start && terminals[state.index] {
            return Some(reconstruct_path(start, state.index, &previous));
        }

        for neighbor in &adjacency[state.index] {
            let spill = state.spill_elevation.max(elevations[neighbor.index]);
            if spill + f32::EPSILON >= best[neighbor.index] {
                continue;
            }
            best[neighbor.index] = spill;
            previous[neighbor.index] = Some(state.index);
            heap.push(SpillState {
                index: neighbor.index,
                spill_elevation: spill,
                path_len: state.path_len.saturating_add(1),
            });
        }
    }

    None
}

fn reconstruct_path(start: usize, end: usize, previous: &[Option<usize>]) -> Vec<usize> {
    let mut path = Vec::new();
    let mut current = end;
    path.push(current);

    while current != start {
        let Some(prev) = previous[current] else {
            break;
        };
        current = prev;
        path.push(current);
    }

    path.reverse();
    path
}

fn edge_between(
    from: usize,
    to: usize,
    adjacency: &[Vec<CornerNeighbor>],
) -> Option<VoronoiEdgeId> {
    adjacency[from]
        .iter()
        .find(|neighbor| neighbor.index == to)
        .map(|neighbor| neighbor.edge)
}

fn break_remaining_cycles(
    downstream: &mut [Option<usize>],
    downstream_edges: &mut [Option<VoronoiEdgeId>],
    resolutions: &mut [GraphLocalMinimumResolution],
) {
    for start in 0..downstream.len() {
        let mut seen = HashMap::new();
        let mut current = start;

        while let Some(next) = downstream[current] {
            if let Some(&cycle_at) = seen.get(&next) {
                downstream[cycle_at] = None;
                downstream_edges[cycle_at] = None;
                resolutions[cycle_at] = GraphLocalMinimumResolution::Sink;
                break;
            }
            seen.insert(current, current);
            current = next;
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct LakeContactTopology {
    component_by_corner: Vec<Option<usize>>,
    contact_component_by_land_corner: Vec<Option<usize>>,
    inlet_vertices: Vec<bool>,
    outlet_vertices: Vec<bool>,
    inlet_land_vertices: Vec<bool>,
    outlet_land_vertices: Vec<bool>,
}

fn resolve_lake_contact_topology(
    downstream: &mut [Option<usize>],
    downstream_edges: &mut [Option<VoronoiEdgeId>],
    resolutions: &mut [GraphLocalMinimumResolution],
    elevations: &[f32],
    lake_candidates: &[bool],
    adjacency: &[Vec<CornerNeighbor>],
    edge_map: &HashMap<VoronoiEdgeId, MacroEdge>,
    config: HydrologyConfig,
) -> LakeContactTopology {
    let components = lake_components(lake_candidates, adjacency);
    let mut topology = LakeContactTopology {
        component_by_corner: vec![None; lake_candidates.len()],
        contact_component_by_land_corner: vec![None; lake_candidates.len()],
        inlet_vertices: vec![false; lake_candidates.len()],
        outlet_vertices: vec![false; lake_candidates.len()],
        inlet_land_vertices: vec![false; lake_candidates.len()],
        outlet_land_vertices: vec![false; lake_candidates.len()],
    };

    for (component_index, component) in components.iter().enumerate() {
        for &corner in component {
            topology.component_by_corner[corner] = Some(component_index);
        }
    }

    let mut component_inlet_elevation = vec![f32::NEG_INFINITY; components.len()];
    for (from, target) in downstream.iter().copied().enumerate() {
        let Some(target) = target else {
            continue;
        };
        if !lake_candidates[from] && lake_candidates[target] {
            topology.inlet_vertices[target] = true;
            topology.inlet_land_vertices[from] = true;
            if let Some(component) = topology.component_by_corner[target] {
                topology.contact_component_by_land_corner[from] = Some(component);
                component_inlet_elevation[component] =
                    component_inlet_elevation[component].max(elevations[from]);
            }
        }
    }

    for (component_index, component) in components.iter().enumerate() {
        if config.lake_max_outlets_per_component == 0 {
            continue;
        }
        let inlet_vertices = component
            .iter()
            .copied()
            .filter(|&index| topology.inlet_vertices[index])
            .collect::<Vec<_>>();
        if inlet_vertices.is_empty() {
            continue;
        }
        let inlet_elevation = component_inlet_elevation[component_index];
        if !inlet_elevation.is_finite() {
            continue;
        }
        let inlet_distances =
            lake_hop_distances(component, &inlet_vertices, lake_candidates, adjacency);
        let Some((outlet, target, edge, outlet_next, outlet_edge)) = choose_lake_outlet(
            component,
            inlet_elevation,
            &inlet_distances,
            elevations,
            lake_candidates,
            adjacency,
            edge_map,
            config,
        ) else {
            continue;
        };

        downstream[outlet] = Some(target);
        downstream_edges[outlet] = Some(edge);
        downstream[target] = Some(outlet_next);
        downstream_edges[target] = Some(outlet_edge);
        topology.outlet_land_vertices[target] = true;
        topology.contact_component_by_land_corner[target] = Some(component_index);
        if resolutions[outlet] == GraphLocalMinimumResolution::Lake {
            resolutions[outlet] = GraphLocalMinimumResolution::OutletCarve;
        }
        topology.outlet_vertices[outlet] = true;
    }

    topology
}

fn lake_components(lake_candidates: &[bool], adjacency: &[Vec<CornerNeighbor>]) -> Vec<Vec<usize>> {
    let mut components = Vec::new();
    let mut visited = vec![false; lake_candidates.len()];

    for start in 0..lake_candidates.len() {
        if !lake_candidates[start] || visited[start] {
            continue;
        }

        let mut component = Vec::new();
        let mut queue = VecDeque::from([start]);
        visited[start] = true;

        while let Some(index) = queue.pop_front() {
            component.push(index);
            for neighbor in &adjacency[index] {
                if lake_candidates[neighbor.index] && !visited[neighbor.index] {
                    visited[neighbor.index] = true;
                    queue.push_back(neighbor.index);
                }
            }
        }

        component.sort_unstable();
        components.push(component);
    }

    components
}

fn lake_hop_distances(
    component: &[usize],
    starts: &[usize],
    lake_candidates: &[bool],
    adjacency: &[Vec<CornerNeighbor>],
) -> Vec<u32> {
    let mut distances = vec![u32::MAX; lake_candidates.len()];
    let mut queue = VecDeque::new();

    for &start in starts {
        distances[start] = 0;
        queue.push_back(start);
    }

    while let Some(index) = queue.pop_front() {
        let next_distance = distances[index].saturating_add(1);
        for neighbor in &adjacency[index] {
            if !lake_candidates[neighbor.index] || distances[neighbor.index] <= next_distance {
                continue;
            }
            distances[neighbor.index] = next_distance;
            queue.push_back(neighbor.index);
        }
    }

    for &index in component {
        if distances[index] == u32::MAX {
            distances[index] = 0;
        }
    }

    distances
}

fn choose_lake_outlet(
    component: &[usize],
    inlet_elevation: f32,
    inlet_distances: &[u32],
    elevations: &[f32],
    lake_candidates: &[bool],
    adjacency: &[Vec<CornerNeighbor>],
    edge_map: &HashMap<VoronoiEdgeId, MacroEdge>,
    config: HydrologyConfig,
) -> Option<(usize, usize, VoronoiEdgeId, usize, VoronoiEdgeId)> {
    component
        .iter()
        .copied()
        .filter(|&index| {
            inlet_distances[index] >= config.lake_inlet_outlet_min_edge_hops
                && elevations[index] < inlet_elevation - 0.001
        })
        .filter_map(|index| {
            adjacency[index]
                .iter()
                .copied()
                .filter(|neighbor| !lake_candidates[neighbor.index])
                .filter(|neighbor| elevations[neighbor.index] < inlet_elevation - 0.001)
                .filter_map(|neighbor| {
                    let outlet_segment = adjacency[neighbor.index]
                        .iter()
                        .copied()
                        .filter(|next| !lake_candidates[next.index])
                        .filter(|next| elevations[next.index] < elevations[neighbor.index] - 0.001)
                        .filter(|next| !is_lake_edge(next.edge, edge_map))
                        .min_by(|left, right| {
                            elevations[left.index]
                                .total_cmp(&elevations[right.index])
                                .then_with(|| left.index.cmp(&right.index))
                        })?;
                    Some((neighbor, outlet_segment))
                })
                .min_by(|left, right| {
                    elevations[left.0.index]
                        .total_cmp(&elevations[right.0.index])
                        .then_with(|| left.0.index.cmp(&right.0.index))
                })
                .map(|(neighbor, outlet_segment)| {
                    (
                        index,
                        neighbor.index,
                        neighbor.edge,
                        outlet_segment.index,
                        outlet_segment.edge,
                    )
                })
        })
        .min_by(|left, right| {
            elevations[left.0]
                .total_cmp(&elevations[right.0])
                .then_with(|| right.0.cmp(&left.0))
        })
}

fn resolve_watersheds(
    downstream: &[Option<usize>],
    resolutions: &[GraphLocalMinimumResolution],
) -> Vec<WatershedId> {
    let mut watersheds = vec![WatershedId(0); downstream.len()];

    for index in 0..downstream.len() {
        let outlet = downstream_outlet(index, downstream);
        let namespace = match resolutions[outlet] {
            GraphLocalMinimumResolution::OceanOutlet => 0x8b59_c235_426e_1f5d,
            GraphLocalMinimumResolution::OutletCarve => 0xf0a7_4c21_619d_8b63,
            GraphLocalMinimumResolution::Lake => 0x5cf2_d36b_a371_04a5,
            GraphLocalMinimumResolution::Sink => 0x23e1_d19a_74bf_9c42,
            GraphLocalMinimumResolution::None => 0x6a09_e667_f3bc_c909,
        };
        watersheds[index] = WatershedId(splitmix64(outlet as u64 ^ namespace));
    }

    watersheds
}

fn downstream_outlet(start: usize, downstream: &[Option<usize>]) -> usize {
    let mut current = start;
    let mut guard = 0;

    while let Some(next) = downstream[current] {
        current = next;
        guard += 1;
        if guard > downstream.len() {
            return start;
        }
    }

    current
}

fn resolve_terminal_indices(downstream: &[Option<usize>]) -> Vec<usize> {
    (0..downstream.len())
        .into_par_iter()
        .map(|index| downstream_outlet(index, downstream))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LakeTerminalPolicy {
    area_units: u32,
    component_root: usize,
    max_incoming_chains: usize,
    selection_threshold: f32,
    discharge_floor: f32,
    discharge_cap: f32,
}

fn resolve_lake_terminal_policies(
    terminal_indices: &[usize],
    lake_candidates: &[bool],
    resolutions: &[GraphLocalMinimumResolution],
    config: HydrologyConfig,
) -> Vec<Option<LakeTerminalPolicy>> {
    let mut lake_area_units = vec![0_u32; terminal_indices.len()];

    for (index, &terminal) in terminal_indices.iter().enumerate() {
        if resolutions[terminal] != GraphLocalMinimumResolution::Lake {
            continue;
        }
        let contribution = if lake_candidates[index] { 1 } else { 0 };
        lake_area_units[terminal] = lake_area_units[terminal].saturating_add(contribution);
    }

    let mut policies = vec![None; terminal_indices.len()];
    for (index, &area_units) in lake_area_units.iter().enumerate() {
        if resolutions[index] != GraphLocalMinimumResolution::Lake {
            continue;
        }
        let area_units = area_units.max(1);
        let area = area_units as f32;
        let max_incoming_chains = (1.0 + (area / config.lake_area_units_per_chain).floor())
            .clamp(1.0, config.lake_max_incoming_chains as f32)
            as usize;
        let discharge_cap = (config.lake_discharge_cap_floor
            + area * config.lake_discharge_cap_per_area)
            .min(config.lake_discharge_cap_ceiling);
        let selection_threshold = lake_inlet_selection_threshold(area, discharge_cap, config);
        policies[index] = Some(LakeTerminalPolicy {
            area_units,
            component_root: index,
            max_incoming_chains,
            selection_threshold,
            discharge_floor: lake_discharge_floor_for_area(area, config),
            discharge_cap,
        });
    }

    policies
}

fn resolve_lake_inlet_policies(
    lake_candidates: &[bool],
    adjacency: &[Vec<CornerNeighbor>],
    config: HydrologyConfig,
) -> Vec<Option<LakeTerminalPolicy>> {
    let mut policies = vec![None; lake_candidates.len()];
    let mut visited = vec![false; lake_candidates.len()];

    for start in 0..lake_candidates.len() {
        if !lake_candidates[start] || visited[start] {
            continue;
        }

        let mut component = Vec::new();
        let mut queue = VecDeque::from([start]);
        visited[start] = true;

        while let Some(index) = queue.pop_front() {
            component.push(index);
            for neighbor in &adjacency[index] {
                if lake_candidates[neighbor.index] && !visited[neighbor.index] {
                    visited[neighbor.index] = true;
                    queue.push_back(neighbor.index);
                }
            }
        }

        let area_units = component.len().max(1) as u32;
        let root = component.iter().copied().min().unwrap_or(start);
        let policy = lake_policy_for_area(area_units, root, config);
        for index in component {
            policies[index] = Some(policy);
        }
    }

    policies
}

fn lake_policy_for_area(
    area_units: u32,
    component_root: usize,
    config: HydrologyConfig,
) -> LakeTerminalPolicy {
    let area = area_units as f32;
    let max_incoming_chains = (1.0 + (area / config.lake_area_units_per_chain).floor())
        .clamp(1.0, config.lake_max_incoming_chains as f32) as usize;
    let discharge_cap = (config.lake_discharge_cap_floor
        + area * config.lake_discharge_cap_per_area)
        .min(config.lake_discharge_cap_ceiling);
    let selection_threshold = lake_inlet_selection_threshold(area, discharge_cap, config);

    LakeTerminalPolicy {
        area_units,
        component_root,
        max_incoming_chains,
        selection_threshold,
        discharge_floor: lake_discharge_floor_for_area(area, config),
        discharge_cap,
    }
}

fn lake_discharge_floor_for_area(area_units: f32, config: HydrologyConfig) -> f32 {
    (config.lake_discharge_cap_floor * 0.55 + area_units * config.lake_discharge_range_per_area)
        .min(config.lake_discharge_cap_ceiling * 0.55)
}

fn lake_inlet_selection_threshold(
    area_units: f32,
    discharge_cap: f32,
    config: HydrologyConfig,
) -> f32 {
    let area_scale = (area_units / config.lake_area_units_per_chain).sqrt();
    (config.river_flow_threshold * (3.0 + area_scale)).max(discharge_cap * 1.35)
}

fn resolve_first_downstream_lakes(
    downstream: &[Option<usize>],
    lake_candidates: &[bool],
) -> Vec<Option<usize>> {
    (0..downstream.len())
        .into_par_iter()
        .map(|start| {
            let mut current = start;
            let mut guard = 0;

            loop {
                if lake_candidates[current] {
                    return Some(current);
                }
                let Some(next) = downstream[current] else {
                    return None;
                };
                current = next;
                guard += 1;
                if guard > downstream.len() {
                    return None;
                }
            }
        })
        .collect()
}

fn resolve_flow_accumulation(
    downstream: &[Option<usize>],
    terminals: &[bool],
    macro_map: &GraphMacroMap,
    patch: &VoronoiGraphPatch,
) -> Vec<f32> {
    let macro_corners = macro_map
        .corners
        .iter()
        .map(|corner| (corner.id, *corner))
        .collect::<HashMap<_, _>>();
    let mut flow = patch
        .corners
        .par_iter()
        .enumerate()
        .map(|(index, corner)| {
            if terminals[index] {
                return 0.0;
            }
            macro_corners
                .get(&corner.id)
                .map(|macro_corner| {
                    if macro_corner.surface_kind.is_land_owned() {
                        (0.55 + macro_corner.basinness * 0.30 + macro_corner.coastness * 0.15)
                            .max(0.10)
                    } else {
                        0.0
                    }
                })
                .unwrap_or(0.0)
        })
        .collect::<Vec<_>>();
    let mut incoming = vec![0_u32; downstream.len()];

    for target in downstream.iter().flatten().copied() {
        incoming[target] = incoming[target].saturating_add(1);
    }

    let mut queue = incoming
        .iter()
        .enumerate()
        .filter_map(|(index, &count)| (count == 0).then_some(index))
        .collect::<VecDeque<_>>();

    while let Some(index) = queue.pop_front() {
        if let Some(target) = downstream[index] {
            flow[target] += flow[index];
            incoming[target] = incoming[target].saturating_sub(1);
            if incoming[target] == 0 {
                queue.push_back(target);
            }
        }
    }

    flow
}

fn select_river_paths(
    downstream: &[Option<usize>],
    downstream_edges: &[Option<VoronoiEdgeId>],
    flow: &[f32],
    elevations: &[f32],
    terminals: &[bool],
    lake_candidates: &[bool],
    terminal_indices: &[usize],
    first_downstream_lakes: &[Option<usize>],
    lake_policies: &[Option<LakeTerminalPolicy>],
    lake_inlet_policies: &[Option<LakeTerminalPolicy>],
    lake_topology: &LakeContactTopology,
    edge_map: &HashMap<VoronoiEdgeId, MacroEdge>,
    config: HydrologyConfig,
) -> SelectedRiverPaths {
    let mut selected = vec![false; downstream.len()];
    let mut selected_lake_chains = vec![0_usize; downstream.len()];
    let mut candidates = flow
        .iter()
        .enumerate()
        .filter_map(|(index, &amount)| {
            let terminal = terminal_indices[index];
            let lake_policy = lake_policies[terminal].or_else(|| {
                first_downstream_lakes[index].and_then(|lake| lake_inlet_policies[lake])
            });
            let threshold = if let Some(policy) = lake_policy {
                policy.selection_threshold
            } else {
                config.river_flow_threshold
            };
            (amount >= threshold
                && elevations[index] >= config.headwater_elevation
                && downstream_edges[index].is_some_and(|edge| !is_lake_edge(edge, edge_map))
                && !terminals[index]
                && !lake_candidates[index])
                .then_some(index)
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|&left, &right| {
        flow[right]
            .total_cmp(&flow[left])
            .then_with(|| left.cmp(&right))
    });

    for start in candidates {
        let terminal = terminal_indices[start];
        let first_lake = first_downstream_lakes[start];
        let lake_policy = lake_policies[terminal]
            .or_else(|| first_lake.and_then(|lake| lake_inlet_policies[lake]));

        if let Some(policy) = lake_policy {
            if selected_lake_chains[policy.component_root] >= policy.max_incoming_chains {
                continue;
            }
            selected_lake_chains[policy.component_root] += 1;
        }

        let mut chain = Vec::new();
        let mut current = start;
        let mut guard = 0;
        while let Some(next) = downstream[current] {
            if downstream_edges[current].is_some() {
                chain.push(current);
            }
            if first_lake.is_some_and(|lake| next == lake) {
                break;
            }
            if terminals[next] {
                break;
            }
            current = next;
            guard += 1;
            if guard > downstream.len() {
                break;
            }
        }

        for index in chain {
            selected[index] = true;
        }
    }

    enforce_lake_contact_topology(
        &mut selected,
        downstream,
        downstream_edges,
        flow,
        elevations,
        lake_candidates,
        lake_topology,
        lake_inlet_policies,
        edge_map,
    );
    remove_invalid_terminal_intersections(&mut selected, downstream, flow, lake_candidates);
    let duplicate_trunk_pruned_count =
        remove_ambiguous_shared_corner_intersections(&mut selected, downstream, flow);
    let repeated_lake_contact_pruned_count =
        remove_repeated_lake_contact_chains(&mut selected, downstream, lake_topology);
    enforce_lake_contact_topology(
        &mut selected,
        downstream,
        downstream_edges,
        flow,
        elevations,
        lake_candidates,
        lake_topology,
        lake_inlet_policies,
        edge_map,
    );

    SelectedRiverPaths {
        selected,
        duplicate_trunk_pruned_count,
        repeated_lake_contact_pruned_count,
    }
}

fn enforce_lake_contact_topology(
    selected: &mut [bool],
    downstream: &[Option<usize>],
    downstream_edges: &[Option<VoronoiEdgeId>],
    flow: &[f32],
    elevations: &[f32],
    lake_candidates: &[bool],
    lake_topology: &LakeContactTopology,
    lake_inlet_policies: &[Option<LakeTerminalPolicy>],
    edge_map: &HashMap<VoronoiEdgeId, MacroEdge>,
) {
    for index in 0..selected.len() {
        if !selected[index] {
            continue;
        }
        let Some(target) = downstream[index] else {
            selected[index] = false;
            continue;
        };
        let from_lake = lake_candidates[index];
        let to_lake = lake_candidates[target];

        if from_lake
            || to_lake
            || downstream_edges[index].is_none()
            || downstream_edges[index].is_some_and(|edge| is_lake_edge(edge, edge_map))
        {
            selected[index] = false;
        }
    }

    let mut component_has_selected_inlet = vec![false; selected.len()];
    let mut component_selected_inlet_elevation = vec![f32::NEG_INFINITY; selected.len()];
    let mut incoming_selected = vec![0_u32; selected.len()];
    let mut incoming_flow = vec![0.0_f32; selected.len()];
    for (index, is_selected) in selected.iter().copied().enumerate() {
        if !is_selected {
            continue;
        }
        let Some(target) = downstream[index] else {
            continue;
        };
        incoming_selected[target] = incoming_selected[target].saturating_add(1);
        incoming_flow[target] = incoming_flow[target].max(flow[index]);
    }

    for (index, is_inlet_land) in lake_topology
        .inlet_land_vertices
        .iter()
        .copied()
        .enumerate()
    {
        if !is_inlet_land || incoming_selected[index] == 0 {
            continue;
        }
        let Some(lake_vertex) = downstream[index] else {
            continue;
        };
        if let Some(component) = lake_topology.component_by_corner[lake_vertex] {
            let threshold = lake_inlet_policies
                .get(lake_vertex)
                .copied()
                .flatten()
                .map(|policy| policy.selection_threshold)
                .unwrap_or(0.0);
            if incoming_flow[index] < threshold {
                for (source, &is_selected) in selected.to_vec().iter().enumerate() {
                    if is_selected && downstream[source] == Some(index) {
                        selected[source] = false;
                    }
                }
                continue;
            }
            component_has_selected_inlet[component] = true;
            component_selected_inlet_elevation[component] =
                component_selected_inlet_elevation[component].max(elevations[index]);
        }
    }

    for (index, is_outlet) in lake_topology
        .outlet_land_vertices
        .iter()
        .copied()
        .enumerate()
    {
        if !is_outlet {
            continue;
        }
        let has_component_inlet =
            lake_topology
                .component_by_corner
                .iter()
                .enumerate()
                .any(|(lake_corner, component)| {
                    component.is_some_and(|component| {
                        component_has_selected_inlet[component]
                            && elevations[index]
                                < component_selected_inlet_elevation[component] - 0.001
                            && lake_topology.outlet_vertices[lake_corner]
                            && downstream[lake_corner] == Some(index)
                    })
                });
        let selected_outlet_edge_is_valid =
            downstream_edges[index].is_some_and(|edge| !is_lake_edge(edge, edge_map));
        if has_component_inlet
            && downstream[index].is_some()
            && !lake_candidates[index]
            && selected_outlet_edge_is_valid
        {
            selected[index] = true;
        } else {
            selected[index] = false;
        }
    }
}

fn remove_invalid_terminal_intersections(
    selected: &mut [bool],
    downstream: &[Option<usize>],
    flow: &[f32],
    lake_candidates: &[bool],
) {
    let mut incoming = vec![Vec::<usize>::new(); selected.len()];
    let mut has_outgoing = vec![false; selected.len()];

    for (index, is_selected) in selected.iter().copied().enumerate() {
        if !is_selected {
            continue;
        }
        has_outgoing[index] = true;
        if let Some(target) = downstream[index] {
            incoming[target].push(index);
        }
    }

    for (corner, sources) in incoming.into_iter().enumerate() {
        if sources.len() <= 1 || has_outgoing[corner] || lake_candidates[corner] {
            continue;
        }
        let keep = sources
            .iter()
            .copied()
            .max_by(|&left, &right| {
                flow[left]
                    .total_cmp(&flow[right])
                    .then_with(|| right.cmp(&left))
            })
            .expect("non-empty intersection list should have a keep candidate");
        for source in sources {
            if source != keep {
                selected[source] = false;
            }
        }
    }
}

fn remove_ambiguous_shared_corner_intersections(
    selected: &mut [bool],
    downstream: &[Option<usize>],
    flow: &[f32],
) -> usize {
    let mut pruned = 0_usize;

    loop {
        let mut incoming = vec![Vec::<usize>::new(); selected.len()];
        for (index, is_selected) in selected.iter().copied().enumerate() {
            if !is_selected {
                continue;
            }
            if let Some(target) = downstream[index] {
                incoming[target].push(index);
            }
        }

        let Some((_, sources)) = incoming
            .into_iter()
            .enumerate()
            .find(|(_, sources)| sources.len() > 1)
        else {
            break;
        };

        let keep = sources
            .iter()
            .copied()
            .max_by(|&left, &right| {
                flow[left]
                    .total_cmp(&flow[right])
                    .then_with(|| right.cmp(&left))
            })
            .expect("ambiguous shared corner should have at least one selected source");

        for source in sources {
            if source == keep || !selected[source] {
                continue;
            }
            pruned += remove_selected_upstream_tree(source, selected, downstream);
        }
    }

    pruned
}

fn remove_repeated_lake_contact_chains(
    selected: &mut [bool],
    downstream: &[Option<usize>],
    lake_topology: &LakeContactTopology,
) -> usize {
    let mut incoming = vec![0_u32; selected.len()];
    for (index, is_selected) in selected.iter().copied().enumerate() {
        if !is_selected {
            continue;
        }
        if let Some(target) = downstream[index] {
            incoming[target] = incoming[target].saturating_add(1);
        }
    }

    let mut pruned = 0_usize;
    for root in 0..selected.len() {
        if !selected[root] || incoming[root] > 0 {
            continue;
        }

        let mut current = root;
        let mut path = Vec::new();
        let mut seen_lake_contact = lake_topology.contact_component_by_land_corner[current];
        let mut guard = 0;

        while selected[current] {
            path.push(current);
            let Some(next) = downstream[current] else {
                break;
            };

            if let Some(component) = lake_topology.contact_component_by_land_corner[next] {
                if seen_lake_contact.is_some() {
                    for index in path {
                        if selected[index] {
                            selected[index] = false;
                            pruned += 1;
                        }
                    }
                    let mut tail = next;
                    let mut tail_guard = 0;
                    while selected[tail] {
                        selected[tail] = false;
                        pruned += 1;
                        let Some(next_tail) = downstream[tail] else {
                            break;
                        };
                        tail = next_tail;
                        tail_guard += 1;
                        if tail_guard > selected.len() {
                            break;
                        }
                    }
                    break;
                }
                seen_lake_contact = Some(component);
            }

            current = next;
            guard += 1;
            if guard > selected.len() {
                break;
            }
        }
    }

    pruned
}

fn remove_selected_upstream_tree(
    root: usize,
    selected: &mut [bool],
    downstream: &[Option<usize>],
) -> usize {
    let mut incoming = vec![Vec::<usize>::new(); selected.len()];
    for (index, is_selected) in selected.iter().copied().enumerate() {
        if !is_selected {
            continue;
        }
        if let Some(target) = downstream[index] {
            incoming[target].push(index);
        }
    }

    let mut removed = 0_usize;
    let mut stack = vec![root];
    while let Some(index) = stack.pop() {
        if !selected[index] {
            continue;
        }
        selected[index] = false;
        removed += 1;
        stack.extend(incoming[index].iter().copied());
    }

    removed
}

fn resolve_selected_flow_accumulation(
    raw_flow: &[f32],
    downstream: &[Option<usize>],
    terminal_indices: &[usize],
    first_downstream_lakes: &[Option<usize>],
    lake_policies: &[Option<LakeTerminalPolicy>],
    lake_inlet_policies: &[Option<LakeTerminalPolicy>],
    lake_topology: &LakeContactTopology,
    node_kinds: &[GraphDrainageNodeKind],
    config: HydrologyConfig,
) -> Vec<f32> {
    raw_flow
        .par_iter()
        .enumerate()
        .map(|(index, &flow)| {
            let terminal = terminal_indices[index];
            let lake_policy = lake_policies[terminal]
                .or_else(|| {
                    first_downstream_lakes[index].and_then(|lake| lake_inlet_policies[lake])
                })
                .or_else(|| {
                    downstream[index]
                        .filter(|&target| lake_topology.inlet_land_vertices[target])
                        .and_then(|target| downstream[target])
                        .and_then(|lake| lake_inlet_policies[lake])
                });

            if let Some(policy) = lake_policy {
                if flow >= policy.selection_threshold {
                    flow.min(policy.discharge_cap).max(policy.discharge_floor)
                } else {
                    flow.min(policy.discharge_cap)
                }
            } else if downstream[index].is_some_and(|target| {
                node_kinds
                    .get(target)
                    .copied()
                    .is_some_and(|kind| kind == GraphDrainageNodeKind::LakeInlet)
            }) {
                flow.min(config.lake_discharge_cap_ceiling)
            } else {
                flow
            }
        })
        .collect()
}

fn resolve_node_kinds(
    selected: &[bool],
    downstream: &[Option<usize>],
    flow: &[f32],
    terminals: &[bool],
    resolutions: &[GraphLocalMinimumResolution],
    lake_topology: &LakeContactTopology,
    lake_inlet_policies: &[Option<LakeTerminalPolicy>],
    config: HydrologyConfig,
) -> Vec<GraphDrainageNodeKind> {
    let mut incoming_selected = vec![0_u32; selected.len()];
    let mut incoming_flow = vec![0.0_f32; selected.len()];

    for (index, is_selected) in selected.iter().copied().enumerate() {
        if !is_selected {
            continue;
        }
        if let Some(target) = downstream[index] {
            incoming_selected[target] = incoming_selected[target].saturating_add(1);
            incoming_flow[target] = incoming_flow[target].max(flow[index]);
        }
    }

    (0..selected.len())
        .into_par_iter()
        .map(|index| {
            let inlet_threshold = downstream[index]
                .and_then(|lake| lake_inlet_policies.get(lake).copied().flatten())
                .map(|policy| policy.selection_threshold)
                .unwrap_or(
                    config.river_flow_threshold * config.lake_river_flow_threshold_multiplier,
                );
            if lake_topology.outlet_land_vertices[index] && selected[index] {
                GraphDrainageNodeKind::LakeOutlet
            } else if lake_topology.inlet_land_vertices[index]
                && incoming_selected[index] > 0
                && incoming_flow[index] >= inlet_threshold
            {
                GraphDrainageNodeKind::LakeInlet
            } else if (terminals[index]
                || resolutions[index] == GraphLocalMinimumResolution::OceanOutlet)
                && incoming_selected[index] > 0
            {
                GraphDrainageNodeKind::CoastOutlet
            } else if matches!(resolutions[index], GraphLocalMinimumResolution::Lake) {
                GraphDrainageNodeKind::Lake
            } else if matches!(resolutions[index], GraphLocalMinimumResolution::Sink) {
                GraphDrainageNodeKind::Sink
            } else if incoming_selected[index] > 1 {
                GraphDrainageNodeKind::Confluence
            } else {
                GraphDrainageNodeKind::Source
            }
        })
        .collect()
}

fn resolve_topology_stats(
    selected: &[bool],
    downstream: &[Option<usize>],
    lake_candidates: &[bool],
    resolutions: &[GraphLocalMinimumResolution],
    downstream_edges: &[Option<VoronoiEdgeId>],
    edge_map: &HashMap<VoronoiEdgeId, MacroEdge>,
    node_kinds: &[GraphDrainageNodeKind],
    duplicate_trunk_pruned_count: usize,
    repeated_lake_contact_pruned_count: usize,
) -> GraphHydrologyTopologyStats {
    let mut stats = GraphHydrologyTopologyStats::default();
    stats.duplicate_trunk_pruned_count = duplicate_trunk_pruned_count;
    stats.repeated_lake_contact_pruned_count = repeated_lake_contact_pruned_count;
    let mut incoming = vec![0_u32; selected.len()];
    let mut outgoing = vec![0_u32; selected.len()];

    for (index, is_selected) in selected.iter().copied().enumerate() {
        if !is_selected {
            continue;
        }
        let Some(target) = downstream[index] else {
            stats.invalid_river_intersection_count += 1;
            continue;
        };
        outgoing[index] = outgoing[index].saturating_add(1);
        incoming[target] = incoming[target].saturating_add(1);

        let edge_is_lake = downstream_edges[index].is_some_and(|edge| is_lake_edge(edge, edge_map));
        let from_lake = lake_candidates[index];
        let to_lake = lake_candidates[target];
        if from_lake || to_lake || edge_is_lake {
            stats.selected_lake_edge_segment_count += 1;
            stats.invalid_lake_contact_count += 1;
        }
    }

    for index in 0..selected.len() {
        if node_kinds[index] == GraphDrainageNodeKind::LakeInlet {
            if incoming[index] > 0 {
                stats.lake_inlet_count += 1;
            } else {
                stats.disconnected_lake_inlet_count += 1;
            }
        }
        if node_kinds[index] == GraphDrainageNodeKind::LakeOutlet {
            if outgoing[index] > 0 {
                stats.lake_outlet_count += 1;
            } else {
                stats.disconnected_lake_outlet_count += 1;
            }
        }
        let touches_lake_component = incoming[index] > 0
            && matches!(
                node_kinds[index],
                GraphDrainageNodeKind::Source
                    | GraphDrainageNodeKind::Confluence
                    | GraphDrainageNodeKind::Lake
            );
        if touches_lake_component
            && !lake_candidates[index]
            && downstream[index].is_some_and(|target| lake_candidates[target])
        {
            stats.unclassified_lake_connected_flow_count += 1;
        }
        if selected[index]
            && !lake_candidates[index]
            && downstream[index].is_some()
            && matches!(
                node_kinds[index],
                GraphDrainageNodeKind::Source | GraphDrainageNodeKind::Confluence
            )
            && downstream[index].is_some_and(|target| {
                lake_candidates[target] || node_kinds[target] == GraphDrainageNodeKind::Lake
            })
        {
            stats.unclassified_lake_connected_flow_count += 1;
        }
    }

    for index in 0..selected.len() {
        if incoming[index] <= 1 {
            continue;
        }
        let is_valid_confluence = outgoing[index] == 1;
        let is_explicit_terminal = matches!(
            resolutions[index],
            GraphLocalMinimumResolution::OceanOutlet
                | GraphLocalMinimumResolution::OutletCarve
                | GraphLocalMinimumResolution::Lake
                | GraphLocalMinimumResolution::Sink
        );
        if !is_valid_confluence && !is_explicit_terminal {
            stats.invalid_river_intersection_count += 1;
        }
        if lake_candidates[index] && incoming[index] > 1 {
            stats.invalid_river_intersection_count += incoming[index].saturating_sub(1) as usize;
        }
        if incoming[index] > 1 {
            stats.ambiguous_shared_corner_count += incoming[index].saturating_sub(1) as usize;
        }
    }

    stats
}

fn is_lake_edge(edge: VoronoiEdgeId, edge_map: &HashMap<VoronoiEdgeId, MacroEdge>) -> bool {
    edge_map
        .get(&edge)
        .is_some_and(|edge| edge.lake_class.excludes_selected_river())
}

fn build_nodes(
    patch: &VoronoiGraphPatch,
    watersheds: &[WatershedId],
    node_kinds: &[GraphDrainageNodeKind],
) -> Vec<GraphDrainageNode> {
    let mut nodes = patch
        .corners
        .par_iter()
        .enumerate()
        .map(|(index, corner)| GraphDrainageNode {
            id: node_id(corner.id),
            kind: node_kinds[index],
            corner: corner.id,
            position: corner.position,
            watershed: watersheds[index],
        })
        .collect::<Vec<_>>();
    nodes.sort_by_key(|node| node.id.0);
    nodes
}

fn build_segments(
    selected: &[bool],
    downstream: &[Option<usize>],
    downstream_edges: &[Option<VoronoiEdgeId>],
    flow: &[f32],
    selected_flow: &[f32],
    elevations: &[f32],
    watersheds: &[WatershedId],
    patch: &VoronoiGraphPatch,
) -> Vec<GraphRiverSegment> {
    let mut segments = selected
        .par_iter()
        .enumerate()
        .filter_map(|(index, &is_selected)| {
            if !is_selected {
                return None;
            }
            let target = downstream[index]?;
            let edge = downstream_edges[index]?;
            let drop = elevations[index] - elevations[target];
            Some(GraphRiverSegment {
                id: GraphRiverSegmentId(splitmix64(
                    edge.0 ^ patch.corners[index].id.0.rotate_left(17),
                )),
                edge,
                from: node_id(patch.corners[index].id),
                to: node_id(patch.corners[target].id),
                watershed: watersheds[index],
                role: river_role(selected_flow[index]),
                raw_flow_accumulation: flow[index],
                flow_accumulation: selected_flow[index],
                downstream_progress: drop.max(0.005),
            })
        })
        .collect::<Vec<_>>();
    segments.sort_by_key(|segment| segment.id.0);
    segments
}

fn river_role(flow: f32) -> GraphHydrologyRole {
    if flow >= 80.0 {
        GraphHydrologyRole::Floodplain
    } else if flow >= 36.0 {
        GraphHydrologyRole::Trunk
    } else if flow >= 18.0 {
        GraphHydrologyRole::Tributary
    } else {
        GraphHydrologyRole::Headwater
    }
}

fn node_id(corner: VoronoiCornerId) -> GraphDrainageNodeId {
    GraphDrainageNodeId(splitmix64(corner.0 ^ 0x75a7_2f38_1e91_4d0c))
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generation::graph::{
        DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS, VoronoiGraphConfig,
        VoronoiGraphPatchRequest, generate_voronoi_graph_patch,
    };
    use crate::world::generation::macro_map::{
        MacroCorner, MacroMapConfig, MacroSurfaceKind, generate_macro_map,
    };
    use std::collections::HashMap;

    #[test]
    fn segments_can_be_queried_by_voronoi_edge() {
        let target_edge = VoronoiEdgeId(7);
        let graph = GraphHydrologyGraph {
            corners: Vec::new(),
            nodes: Vec::new(),
            segments: vec![
                GraphRiverSegment {
                    id: GraphRiverSegmentId(1),
                    edge: target_edge,
                    from: GraphDrainageNodeId(1),
                    to: GraphDrainageNodeId(2),
                    watershed: WatershedId(1),
                    role: GraphHydrologyRole::Trunk,
                    raw_flow_accumulation: 12.0,
                    flow_accumulation: 12.0,
                    downstream_progress: 0.4,
                },
                GraphRiverSegment {
                    id: GraphRiverSegmentId(2),
                    edge: VoronoiEdgeId(8),
                    from: GraphDrainageNodeId(3),
                    to: GraphDrainageNodeId(4),
                    watershed: WatershedId(1),
                    role: GraphHydrologyRole::Tributary,
                    raw_flow_accumulation: 3.0,
                    flow_accumulation: 3.0,
                    downstream_progress: 0.2,
                },
            ],
            topology_stats: GraphHydrologyTopologyStats::default(),
        };

        let matches: Vec<_> = graph.segments_for_edge(target_edge).collect();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, GraphRiverSegmentId(1));
    }

    #[test]
    fn hydrology_solve_is_deterministic() {
        let (patch, macro_map) = test_inputs(42);

        let first = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());
        let second = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());

        assert_eq!(first.corners, second.corners);
        assert_eq!(first.nodes, second.nodes);
        assert_eq!(first.segments, second.segments);
    }

    #[test]
    fn selected_river_segments_have_downstream_continuity() {
        let (patch, macro_map) = test_inputs(91);
        let hydro = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());
        let corners = hydro
            .corners
            .iter()
            .map(|corner| (corner.id, *corner))
            .collect::<HashMap<_, _>>();
        let segments_by_from = hydro
            .segments
            .iter()
            .map(|segment| (segment.from, segment))
            .collect::<HashMap<_, _>>();

        assert!(
            !hydro.segments.is_empty(),
            "default hydrology should select visible river chains"
        );

        for segment in &hydro.segments {
            assert!(
                segment.downstream_progress > 0.0,
                "selected river segments should retain downstream progress"
            );
            let to_corner = hydro
                .nodes
                .iter()
                .find(|node| node.id == segment.to)
                .and_then(|node| corners.get(&node.corner))
                .copied()
                .expect("segment target node should have hydrology corner");

            if matches!(
                to_corner.resolution,
                GraphLocalMinimumResolution::OceanOutlet
                    | GraphLocalMinimumResolution::Lake
                    | GraphLocalMinimumResolution::Sink
            ) {
                continue;
            }

            assert!(
                segments_by_from.contains_key(&segment.to),
                "selected segment should continue to an outlet or explicit resolution"
            );
        }
    }

    #[test]
    fn local_minima_are_explicitly_resolved() {
        let (patch, macro_map) = test_inputs(123);
        let hydro = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());

        for corner in hydro
            .corners
            .iter()
            .filter(|corner| corner.is_local_minimum)
        {
            assert_ne!(corner.resolution, GraphLocalMinimumResolution::None);
        }
    }

    #[test]
    fn seed_42_inland_water_produces_lake_resolution() {
        let (patch, macro_map) = seed_42_preview_inputs();
        let hydro = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());
        let lake_resolutions = hydro
            .corners
            .iter()
            .filter(|corner| corner.resolution == GraphLocalMinimumResolution::Lake)
            .count();
        let lake_nodes = hydro
            .nodes
            .iter()
            .filter(|node| node.kind == GraphDrainageNodeKind::Lake)
            .count();
        let lake_capped_segments = hydro
            .segments
            .iter()
            .filter(|segment| segment.raw_flow_accumulation > segment.flow_accumulation + 0.001)
            .count();

        assert!(
            lake_resolutions >= 2,
            "seed 42 default macro preview window should resolve inland water as lakes, got {lake_resolutions}"
        );
        assert_eq!(lake_nodes, lake_resolutions);
        assert!(
            lake_capped_segments > 0,
            "lake-bound selected rivers should use lake capacity caps"
        );
    }

    #[test]
    fn selected_lake_contacts_use_vertices_not_lake_edges() {
        let (patch, macro_map) = seed_42_preview_inputs();
        let hydro = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());
        let macro_corners = macro_corners_by_id(&macro_map);
        let macro_edges = macro_map
            .edges
            .iter()
            .map(|edge| (edge.id, *edge))
            .collect::<HashMap<_, _>>();
        let nodes = nodes_by_id(&hydro);

        assert!(
            hydro.topology_stats.lake_inlet_count > 0,
            "seed 42 should expose selected lake inlet contacts"
        );
        assert_eq!(hydro.topology_stats.invalid_lake_contact_count, 0);
        assert_eq!(hydro.topology_stats.selected_lake_edge_segment_count, 0);
        assert_eq!(hydro.topology_stats.disconnected_lake_inlet_count, 0);
        assert_eq!(hydro.topology_stats.disconnected_lake_outlet_count, 0);

        for segment in &hydro.segments {
            assert!(
                !macro_edges[&segment.edge]
                    .lake_class
                    .excludes_selected_river(),
                "selected river segment must not use explicit lake edge class: {:?}",
                segment
            );
            let from = &nodes[&segment.from];
            let to = &nodes[&segment.to];
            let from_lake = is_lake_corner(macro_corners[&from.corner]);
            let to_lake = is_lake_corner(macro_corners[&to.corner]);

            assert!(
                !from_lake && !to_lake,
                "selected river segment must stay off lake and lake-boundary edges: {:?}",
                segment
            );
        }
    }

    #[test]
    fn lake_outlet_vertices_are_lower_and_separated_from_inlets() {
        let (patch, macro_map) = seed_42_preview_inputs();
        let hydro = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());
        let corner_indices = patch
            .corners
            .iter()
            .enumerate()
            .map(|(index, corner)| (corner.id, index))
            .collect::<HashMap<_, _>>();
        let adjacency = corner_adjacency(&patch, &corner_indices);
        let macro_corners = macro_corners_by_id(&macro_map);
        let lake_candidates = patch
            .corners
            .iter()
            .map(|corner| is_lake_corner(macro_corners[&corner.id]))
            .collect::<Vec<_>>();
        let components = lake_components(&lake_candidates, &adjacency);
        let component_by_corner = component_by_corner(&components, lake_candidates.len());
        let hydro_corners = hydro
            .corners
            .iter()
            .map(|corner| (corner.id, corner))
            .collect::<HashMap<_, _>>();
        let nodes = nodes_by_id(&hydro);
        let inlet_lake_vertices = hydro
            .segments
            .iter()
            .filter_map(|segment| {
                let to = &nodes[&segment.to];
                let from = &nodes[&segment.from];
                if to.kind != GraphDrainageNodeKind::LakeInlet {
                    return None;
                }
                let inlet_index = corner_indices[&to.corner];
                let lake_vertex = hydro_corners[&to.corner]
                    .downstream
                    .map(|corner| corner_indices[&corner])?;
                Some((lake_vertex, inlet_index, corner_indices[&from.corner]))
            })
            .collect::<Vec<_>>();

        for segment in &hydro.segments {
            let outlet_node = &nodes[&segment.from];
            if outlet_node.kind != GraphDrainageNodeKind::LakeOutlet {
                continue;
            }
            let outlet_index = corner_indices[&outlet_node.corner];
            assert!(
                !lake_candidates[outlet_index],
                "lake outlet marker should be on the selected land-side endpoint"
            );
            let outlet_lake_vertices = adjacency
                .iter()
                .enumerate()
                .filter(|(lake_vertex, _)| {
                    lake_candidates[*lake_vertex]
                        && hydro_corners[&patch.corners[*lake_vertex].id].downstream
                            == Some(outlet_node.corner)
                })
                .map(|(lake_vertex, _)| lake_vertex)
                .collect::<Vec<_>>();
            assert!(
                !outlet_lake_vertices.is_empty(),
                "lake outlet marker should be paired with an adjacent lake vertex"
            );
            let component = component_by_corner[outlet_lake_vertices[0]]
                .expect("lake outlet pair should belong to a lake component");
            let component_inlets = inlet_lake_vertices
                .iter()
                .copied()
                .map(|(lake_vertex, _, _)| lake_vertex)
                .filter(|&lake_vertex| component_by_corner[lake_vertex] == Some(component))
                .collect::<Vec<_>>();
            let component_approaches = inlet_lake_vertices
                .iter()
                .copied()
                .filter(|(lake_vertex, _, _)| component_by_corner[*lake_vertex] == Some(component))
                .map(|(_, _, approach)| approach)
                .collect::<Vec<_>>();
            assert!(
                !component_inlets.is_empty(),
                "lake outlet should only be selected for a lake with selected inlets"
            );
            let distances = lake_hop_distances(
                &components[component],
                &component_inlets,
                &lake_candidates,
                &adjacency,
            );
            let inlet_elevation = component_approaches
                .iter()
                .map(|&index| hydro_corners[&patch.corners[index].id].elevation)
                .fold(f32::NEG_INFINITY, f32::max);
            let outlet_elevation = hydro_corners[&outlet_node.corner].elevation;

            assert!(
                outlet_elevation < inlet_elevation - 0.001,
                "lake outlet elevation {outlet_elevation} should be below inlet elevation {inlet_elevation}"
            );
            assert!(
                outlet_lake_vertices
                    .iter()
                    .any(|&lake_vertex| distances[lake_vertex]
                        >= DEFAULT_LAKE_INLET_OUTLET_MIN_EDGE_HOPS),
                "lake outlet should be separated from inlet by at least {} lake hops",
                DEFAULT_LAKE_INLET_OUTLET_MIN_EDGE_HOPS
            );
        }
    }

    #[test]
    fn selected_river_graph_has_no_invalid_shared_corner_intersections() {
        let (patch, macro_map) = seed_42_preview_inputs();
        let hydro = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());

        assert_eq!(hydro.topology_stats.invalid_river_intersection_count, 0);
        assert_eq!(hydro.topology_stats.invalid_lake_contact_count, 0);
        assert_eq!(hydro.topology_stats.ambiguous_shared_corner_count, 0);
        assert!(
            hydro.topology_stats.duplicate_trunk_pruned_count > 0,
            "seed 42 should exercise duplicate selected trunk pruning"
        );
        assert_eq!(
            invalid_shared_corner_count(&hydro),
            0,
            "selected river graph should only share corners through downstream confluence/branch logic"
        );
    }

    #[test]
    fn lake_contact_markers_are_selected_segment_endpoints() {
        let (_patch, _macro_map) = seed_42_preview_inputs();
        let hydro = solve_hydrology(&_patch, &_macro_map, HydrologyConfig::default());
        let nodes = nodes_by_id(&hydro);
        let mut incoming = HashMap::<GraphDrainageNodeId, usize>::new();
        let mut outgoing = HashMap::<GraphDrainageNodeId, usize>::new();

        for segment in &hydro.segments {
            *incoming.entry(segment.to).or_default() += 1;
            *outgoing.entry(segment.from).or_default() += 1;
        }

        for node in &hydro.nodes {
            match node.kind {
                GraphDrainageNodeKind::LakeInlet => {
                    assert!(
                        incoming.get(&node.id).copied().unwrap_or(0) > 0,
                        "lake inlet marker should be backed by an incoming selected segment"
                    );
                    assert_eq!(
                        outgoing.get(&node.id).copied().unwrap_or(0),
                        0,
                        "lake inlet should terminate the selected river before the lake boundary edge"
                    );
                }
                GraphDrainageNodeKind::LakeOutlet => {
                    assert!(
                        outgoing.get(&node.id).copied().unwrap_or(0) > 0,
                        "lake outlet marker should be backed by an outgoing selected segment"
                    );
                    assert_eq!(
                        incoming.get(&node.id).copied().unwrap_or(0),
                        0,
                        "lake outlet should start after the lake boundary edge"
                    );
                }
                _ => {}
            }
        }

        assert_eq!(hydro.topology_stats.disconnected_lake_inlet_count, 0);
        assert_eq!(hydro.topology_stats.disconnected_lake_outlet_count, 0);
        assert_eq!(nodes.len(), hydro.nodes.len());
    }

    #[test]
    fn selected_chain_cannot_contact_lake_twice() {
        let downstream = vec![Some(1), Some(2), Some(3), None];
        let mut selected = vec![true, true, true, false];
        let topology = LakeContactTopology {
            component_by_corner: vec![None; 4],
            contact_component_by_land_corner: vec![Some(10), None, Some(11), None],
            inlet_vertices: vec![false; 4],
            outlet_vertices: vec![false; 4],
            inlet_land_vertices: vec![false; 4],
            outlet_land_vertices: vec![true, false, false, false],
        };

        let pruned = remove_repeated_lake_contact_chains(&mut selected, &downstream, &topology);

        assert_eq!(pruned, 3);
        assert!(
            selected.iter().all(|selected| !selected),
            "a selected path that starts at one lake contact and reaches a second lake contact should be removed"
        );
    }

    #[test]
    fn lake_inlet_requires_large_incoming_flow() {
        let selected = vec![true, false];
        let downstream = vec![Some(1), None];
        let flow = vec![
            DEFAULT_RIVER_FLOW_THRESHOLD * DEFAULT_LAKE_RIVER_FLOW_THRESHOLD_MULTIPLIER - 0.5,
            0.0,
        ];
        let topology = LakeContactTopology {
            component_by_corner: vec![None; 2],
            contact_component_by_land_corner: vec![None, Some(1)],
            inlet_vertices: vec![false; 2],
            outlet_vertices: vec![false; 2],
            inlet_land_vertices: vec![false, true],
            outlet_land_vertices: vec![false; 2],
        };
        let kinds = resolve_node_kinds(
            &selected,
            &downstream,
            &flow,
            &[false; 2],
            &[GraphLocalMinimumResolution::None; 2],
            &topology,
            &[None; 2],
            HydrologyConfig::default(),
        );

        assert_ne!(
            kinds[1],
            GraphDrainageNodeKind::LakeInlet,
            "small feeder streams should not be promoted to visible lake inlet markers"
        );
    }

    #[test]
    fn lake_outlet_count_is_limited_per_component() {
        let (_patch, _macro_map) = seed_42_preview_inputs();
        let hydro = solve_hydrology(&_patch, &_macro_map, HydrologyConfig::default());
        let macro_corners = macro_corners_by_id(&_macro_map);
        let corner_indices = _patch
            .corners
            .iter()
            .enumerate()
            .map(|(index, corner)| (corner.id, index))
            .collect::<HashMap<_, _>>();
        let adjacency = corner_adjacency(&_patch, &corner_indices);
        let lake_candidates = _patch
            .corners
            .iter()
            .map(|corner| is_lake_corner(macro_corners[&corner.id]))
            .collect::<Vec<_>>();
        let components = lake_components(&lake_candidates, &adjacency);
        let component_by_corner = component_by_corner(&components, lake_candidates.len());
        let hydro_corners = hydro
            .corners
            .iter()
            .map(|corner| (corner.id, corner))
            .collect::<HashMap<_, _>>();
        let mut outlets_by_component = HashMap::<usize, usize>::new();

        for node in hydro
            .nodes
            .iter()
            .filter(|node| node.kind == GraphDrainageNodeKind::LakeOutlet)
        {
            let Some(lake_corner) = adjacency.iter().enumerate().find_map(|(lake_corner, _)| {
                (lake_candidates[lake_corner]
                    && hydro_corners[&_patch.corners[lake_corner].id].downstream
                        == Some(node.corner))
                .then_some(lake_corner)
            }) else {
                continue;
            };
            if let Some(component) = component_by_corner[lake_corner] {
                *outlets_by_component.entry(component).or_default() += 1;
            }
        }

        assert!(
            outlets_by_component
                .values()
                .all(|&count| count <= DEFAULT_LAKE_MAX_OUTLETS_PER_COMPONENT),
            "lake outlet markers should stay within the documented per-lake cap"
        );
    }

    #[test]
    fn local_minima_do_not_all_become_lakes() {
        let (patch, macro_map) = seed_42_preview_inputs();
        let hydro = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());
        let lake_minima = hydro
            .corners
            .iter()
            .filter(|corner| corner.is_local_minimum)
            .filter(|corner| corner.resolution == GraphLocalMinimumResolution::Lake)
            .count();
        let dry_or_closed_minima = hydro
            .corners
            .iter()
            .filter(|corner| corner.is_local_minimum)
            .filter(|corner| {
                matches!(
                    corner.resolution,
                    GraphLocalMinimumResolution::Sink | GraphLocalMinimumResolution::OutletCarve
                )
            })
            .count();

        assert!(
            lake_minima > 0,
            "seed 42 should still expose explicit lake minima for preview diagnosis"
        );
        assert!(
            dry_or_closed_minima > 0,
            "local minima should not all resolve as lakes; dry/closed sink or outlet carve basins must exist"
        );
    }

    #[test]
    fn lake_connected_selected_flow_is_always_classified() {
        let (patch, macro_map) = seed_42_preview_inputs();
        let hydro = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());

        assert_eq!(
            hydro.topology_stats.unclassified_lake_connected_flow_count, 0,
            "selected flow adjacent to a lake must resolve as either LakeInlet or LakeOutlet"
        );
        assert_eq!(hydro.topology_stats.selected_lake_edge_segment_count, 0);
        assert_eq!(hydro.topology_stats.disconnected_lake_inlet_count, 0);
        assert_eq!(hydro.topology_stats.disconnected_lake_outlet_count, 0);
    }

    #[test]
    fn lake_terminal_policy_limits_selected_incoming_chains_by_area() {
        let downstream = vec![Some(4), Some(5), Some(6), None, Some(3), Some(3), Some(3)];
        let downstream_edges = vec![
            Some(VoronoiEdgeId(10)),
            Some(VoronoiEdgeId(11)),
            Some(VoronoiEdgeId(12)),
            None,
            Some(VoronoiEdgeId(13)),
            Some(VoronoiEdgeId(14)),
            Some(VoronoiEdgeId(15)),
        ];
        let flow = vec![72.0, 70.0, 68.0, 210.0, 72.0, 70.0, 68.0];
        let elevations = vec![0.8, 0.7, 0.6, 0.1, 0.5, 0.45, 0.4];
        let terminals = vec![false, false, false, false, false, false, false];
        let terminal_indices = resolve_terminal_indices(&downstream);
        let lake_candidates = vec![false, false, false, true, false, false, false];
        let resolutions = vec![
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::Lake,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
        ];
        let config = HydrologyConfig::default();
        let lake_policies = resolve_lake_terminal_policies(
            &terminal_indices,
            &lake_candidates,
            &resolutions,
            config,
        );
        let first_downstream_lakes =
            vec![Some(3), Some(3), Some(3), None, Some(3), Some(3), Some(3)];
        let lake_inlet_policies = vec![None; 7];
        let lake_topology = LakeContactTopology {
            component_by_corner: vec![None, None, None, Some(3), None, None, None],
            contact_component_by_land_corner: vec![
                None,
                None,
                None,
                None,
                Some(3),
                Some(3),
                Some(3),
            ],
            inlet_vertices: vec![false, false, false, true, false, false, false],
            outlet_vertices: vec![false; 7],
            inlet_land_vertices: vec![false, false, false, false, true, true, true],
            outlet_land_vertices: vec![false; 7],
        };

        let selected = select_river_paths(
            &downstream,
            &downstream_edges,
            &flow,
            &elevations,
            &terminals,
            &lake_candidates,
            &terminal_indices,
            &first_downstream_lakes,
            &lake_policies,
            &lake_inlet_policies,
            &lake_topology,
            &HashMap::new(),
            config,
        )
        .selected;

        let incoming_to_lake = selected[..3].iter().filter(|&&selected| selected).count();
        assert_eq!(
            incoming_to_lake, 1,
            "a tiny lake should accept only one selected incoming river chain"
        );
        assert!(
            !selected[4] && !selected[5] && !selected[6],
            "selected river must not include lake boundary edges"
        );
    }

    #[test]
    fn lake_terminal_chain_budget_grows_slowly_with_lake_area() {
        let terminal = 40;
        let mut terminal_indices = vec![terminal; 41];
        terminal_indices[terminal] = terminal;
        let resolutions = (0..41)
            .map(|index| {
                if index == terminal {
                    GraphLocalMinimumResolution::Lake
                } else {
                    GraphLocalMinimumResolution::None
                }
            })
            .collect::<Vec<_>>();
        let config = HydrologyConfig::default();

        let mut small_lake = vec![false; 41];
        small_lake[terminal] = true;
        let small_policy =
            resolve_lake_terminal_policies(&terminal_indices, &small_lake, &resolutions, config)
                [terminal]
                .expect("small lake should have policy");

        let mut large_lake = vec![false; 41];
        for is_lake in large_lake.iter_mut().take(37) {
            *is_lake = true;
        }
        large_lake[terminal] = true;
        let large_policy =
            resolve_lake_terminal_policies(&terminal_indices, &large_lake, &resolutions, config)
                [terminal]
                .expect("large lake should have policy");

        assert_eq!(small_policy.area_units, 1);
        assert_eq!(small_policy.max_incoming_chains, 1);
        assert!(
            large_policy.max_incoming_chains > small_policy.max_incoming_chains,
            "larger lake footprint should allow more selected incoming chains"
        );
        assert!(
            large_policy.max_incoming_chains <= DEFAULT_LAKE_MAX_INCOMING_CHAINS,
            "even large lakes should not accept ocean-like river networks"
        );
        assert!(
            large_policy.discharge_cap > small_policy.discharge_cap,
            "larger lake footprint should allow proportionally larger inlet display discharge"
        );
        assert!(
            large_policy.discharge_floor > small_policy.discharge_floor,
            "larger lake footprint should lift the selected/display discharge range, not only the cap"
        );
        assert!(
            large_policy.selection_threshold > small_policy.selection_threshold,
            "larger lakes should require a stronger raw feeder before exposing an inlet marker"
        );
    }

    #[test]
    fn selected_lake_inlet_chain_keeps_upstream_trunk_to_boundary_endpoint() {
        let downstream = vec![Some(1), Some(2), Some(3), None];
        let downstream_edges = vec![
            Some(VoronoiEdgeId(10)),
            Some(VoronoiEdgeId(11)),
            Some(VoronoiEdgeId(12)),
            None,
        ];
        let flow = vec![96.0, 94.0, 92.0, 92.0];
        let elevations = vec![0.8, 0.7, 0.6, 0.2];
        let terminals = vec![false; 4];
        let terminal_indices = resolve_terminal_indices(&downstream);
        let lake_candidates = vec![false, false, false, true];
        let resolutions = vec![
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::Lake,
        ];
        let config = HydrologyConfig::default();
        let lake_policies = resolve_lake_terminal_policies(
            &terminal_indices,
            &lake_candidates,
            &resolutions,
            config,
        );
        let first_downstream_lakes = vec![Some(3), Some(3), Some(3), None];
        let lake_inlet_policies = vec![None; 4];
        let lake_topology = LakeContactTopology {
            component_by_corner: vec![None, None, None, Some(0)],
            contact_component_by_land_corner: vec![None, None, Some(0), None],
            inlet_vertices: vec![false, false, false, true],
            outlet_vertices: vec![false; 4],
            inlet_land_vertices: vec![false, false, true, false],
            outlet_land_vertices: vec![false; 4],
        };

        let selected = select_river_paths(
            &downstream,
            &downstream_edges,
            &flow,
            &elevations,
            &terminals,
            &lake_candidates,
            &terminal_indices,
            &first_downstream_lakes,
            &lake_policies,
            &lake_inlet_policies,
            &lake_topology,
            &HashMap::new(),
            config,
        )
        .selected;

        assert!(
            selected[0] && selected[1],
            "a qualifying lake-bound river should keep its existing upstream trunk selected"
        );
        assert!(
            !selected[2],
            "the lake boundary segment itself should still be removed from selected rivers"
        );
    }

    #[test]
    fn lake_terminal_policy_caps_selected_flow_but_keeps_raw_accumulation() {
        let raw_flow = vec![75.0, 120.0, 220.0, 300.0];
        let terminal_indices = vec![3, 3, 3, 3];
        let lake_candidates = vec![true, false, true, true];
        let resolutions = vec![
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::Lake,
        ];
        let config = HydrologyConfig::default();
        let lake_policies = resolve_lake_terminal_policies(
            &terminal_indices,
            &lake_candidates,
            &resolutions,
            config,
        );

        let first_downstream_lakes = vec![None; 4];
        let lake_inlet_policies = vec![None; 4];
        let lake_topology = LakeContactTopology {
            component_by_corner: vec![None; 4],
            contact_component_by_land_corner: vec![None; 4],
            inlet_vertices: vec![false; 4],
            outlet_vertices: vec![false; 4],
            inlet_land_vertices: vec![false; 4],
            outlet_land_vertices: vec![false; 4],
        };
        let selected_flow = resolve_selected_flow_accumulation(
            &raw_flow,
            &[None; 4],
            &terminal_indices,
            &first_downstream_lakes,
            &lake_policies,
            &lake_inlet_policies,
            &lake_topology,
            &[GraphDrainageNodeKind::Source; 4],
            config,
        );
        let policy = lake_policies[3].expect("lake terminal should have policy");

        assert!(policy.discharge_cap < raw_flow[2]);
        assert!(
            policy.discharge_cap <= DEFAULT_LAKE_DISCHARGE_CAP_CEILING,
            "lake display discharge should stay under the strong lake cap"
        );
        assert_eq!(selected_flow[2], policy.discharge_cap);
        assert_eq!(
            raw_flow[2], 220.0,
            "raw hydrology ledger should remain unchanged"
        );
    }

    #[test]
    fn lake_inlet_endpoint_segment_uses_display_cap() {
        let raw_flow = vec![180.0, 180.0, 0.0];
        let downstream = vec![Some(1), Some(2), None];
        let terminal_indices = vec![2, 2, 2];
        let first_downstream_lakes = vec![None; 3];
        let lake_policies = vec![None; 3];
        let config = HydrologyConfig::default();
        let lake_policy = lake_policy_for_area(24, 2, config);
        let lake_inlet_policies = vec![None, None, Some(lake_policy)];
        let lake_topology = LakeContactTopology {
            component_by_corner: vec![None, None, Some(0)],
            contact_component_by_land_corner: vec![None, Some(0), None],
            inlet_vertices: vec![false, false, true],
            outlet_vertices: vec![false; 3],
            inlet_land_vertices: vec![false, true, false],
            outlet_land_vertices: vec![false; 3],
        };
        let node_kinds = vec![
            GraphDrainageNodeKind::Source,
            GraphDrainageNodeKind::LakeInlet,
            GraphDrainageNodeKind::Lake,
        ];

        let selected_flow = resolve_selected_flow_accumulation(
            &raw_flow,
            &downstream,
            &terminal_indices,
            &first_downstream_lakes,
            &lake_policies,
            &lake_inlet_policies,
            &lake_topology,
            &node_kinds,
            config,
        );

        assert_eq!(
            selected_flow[0], lake_policy.discharge_cap,
            "the segment ending at a LakeInlet marker should use lake-area display discharge"
        );
        assert_eq!(raw_flow[0], 180.0);
    }

    #[test]
    fn lake_terminal_display_flow_cap_is_well_below_ocean_terminal_flow() {
        let raw_flow = vec![12.0, 160.0, 260.0, 400.0, 400.0];
        let terminal_indices = vec![3, 3, 3, 3, 4];
        let lake_candidates = vec![true, true, true, true, false];
        let resolutions = vec![
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::Lake,
            GraphLocalMinimumResolution::OceanOutlet,
        ];
        let config = HydrologyConfig::default();
        let lake_policies = resolve_lake_terminal_policies(
            &terminal_indices,
            &lake_candidates,
            &resolutions,
            config,
        );

        let first_downstream_lakes = vec![None; 5];
        let lake_inlet_policies = vec![None; 5];
        let lake_topology = LakeContactTopology {
            component_by_corner: vec![None; 5],
            contact_component_by_land_corner: vec![None; 5],
            inlet_vertices: vec![false; 5],
            outlet_vertices: vec![false; 5],
            inlet_land_vertices: vec![false; 5],
            outlet_land_vertices: vec![false; 5],
        };
        let selected_flow = resolve_selected_flow_accumulation(
            &raw_flow,
            &[None; 5],
            &terminal_indices,
            &first_downstream_lakes,
            &lake_policies,
            &lake_inlet_policies,
            &lake_topology,
            &[GraphDrainageNodeKind::Source; 5],
            config,
        );

        assert_eq!(
            selected_flow[4], raw_flow[4],
            "ocean terminal flow should not use the lake display cap"
        );
        assert!(
            selected_flow[2] <= raw_flow[4] * 0.05,
            "lake terminal display flow should be visibly below ocean terminal flow"
        );
    }

    fn test_inputs(seed: u64) -> (VoronoiGraphPatch, GraphMacroMap) {
        let patch = generate_voronoi_graph_patch(VoronoiGraphPatchRequest::new(
            VoronoiGraphConfig {
                seed,
                generator_version: 3,
                region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
                site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
                padding_regions: 2,
            },
            0,
            0,
        ));
        let macro_map = generate_macro_map(&patch, MacroMapConfig::new(seed, 3));
        (patch, macro_map)
    }

    fn seed_42_preview_inputs() -> (VoronoiGraphPatch, GraphMacroMap) {
        let patch = generate_voronoi_graph_patch(VoronoiGraphPatchRequest::new(
            VoronoiGraphConfig {
                seed: 42,
                generator_version: 11,
                region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
                site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
                padding_regions: 17,
            },
            0,
            0,
        ));
        let macro_map = generate_macro_map(&patch, MacroMapConfig::new(42, 11));
        (patch, macro_map)
    }

    fn macro_corners_by_id(map: &GraphMacroMap) -> HashMap<VoronoiCornerId, MacroCorner> {
        map.corners
            .iter()
            .map(|corner| (corner.id, *corner))
            .collect()
    }

    fn nodes_by_id(graph: &GraphHydrologyGraph) -> HashMap<GraphDrainageNodeId, GraphDrainageNode> {
        graph
            .nodes
            .iter()
            .map(|node| (node.id, node.clone()))
            .collect()
    }

    fn is_lake_corner(corner: MacroCorner) -> bool {
        matches!(
            corner.surface_kind,
            MacroSurfaceKind::LakeCandidate | MacroSurfaceKind::WetlandCandidate
        )
    }

    fn component_by_corner(components: &[Vec<usize>], len: usize) -> Vec<Option<usize>> {
        let mut by_corner = vec![None; len];
        for (component, corners) in components.iter().enumerate() {
            for &corner in corners {
                by_corner[corner] = Some(component);
            }
        }
        by_corner
    }

    fn invalid_shared_corner_count(graph: &GraphHydrologyGraph) -> usize {
        let mut incoming = HashMap::<GraphDrainageNodeId, usize>::new();
        let mut outgoing = HashMap::<GraphDrainageNodeId, usize>::new();
        let nodes = nodes_by_id(graph);

        for segment in &graph.segments {
            *outgoing.entry(segment.from).or_default() += 1;
            *incoming.entry(segment.to).or_default() += 1;
        }

        incoming
            .into_iter()
            .filter(|(node, count)| {
                if *count <= 1 {
                    return false;
                }
                let outgoing_count = outgoing.get(node).copied().unwrap_or(0);
                let kind = nodes[node].kind;
                outgoing_count != 1
                    && !matches!(
                        kind,
                        GraphDrainageNodeKind::LakeInlet
                            | GraphDrainageNodeKind::Sink
                            | GraphDrainageNodeKind::CoastOutlet
                    )
            })
            .count()
    }
}
