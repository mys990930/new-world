use rayon::prelude::*;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, VecDeque};

use super::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiGraphPatch, WorldPlanePoint};
use super::macro_map::{GraphMacroMap, MacroCorner, MacroSurfaceKind};

pub const DEFAULT_RIVER_FLOW_THRESHOLD: f32 = 10.0;
pub const DEFAULT_HEADWATER_ELEVATION: f32 = 0.10;
pub const DEFAULT_LAKE_RIVER_FLOW_THRESHOLD_MULTIPLIER: f32 = 2.4;
pub const DEFAULT_LAKE_DISCHARGE_CAP_PER_AREA: f32 = 2.75;
pub const DEFAULT_LAKE_DISCHARGE_CAP_FLOOR: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HydrologyConfig {
    pub river_flow_threshold: f32,
    pub headwater_elevation: f32,
    pub lake_river_flow_threshold_multiplier: f32,
    pub lake_discharge_cap_per_area: f32,
    pub lake_discharge_cap_floor: f32,
}

impl Default for HydrologyConfig {
    fn default() -> Self {
        Self {
            river_flow_threshold: DEFAULT_RIVER_FLOW_THRESHOLD,
            headwater_elevation: DEFAULT_HEADWATER_ELEVATION,
            lake_river_flow_threshold_multiplier: DEFAULT_LAKE_RIVER_FLOW_THRESHOLD_MULTIPLIER,
            lake_discharge_cap_per_area: DEFAULT_LAKE_DISCHARGE_CAP_PER_AREA,
            lake_discharge_cap_floor: DEFAULT_LAKE_DISCHARGE_CAP_FLOOR,
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

#[derive(Debug, Clone, Default)]
pub struct GraphHydrologyGraph {
    pub corners: Vec<GraphHydrologyCorner>,
    pub nodes: Vec<GraphDrainageNode>,
    pub segments: Vec<GraphRiverSegment>,
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

    let (downstream, downstream_edges, local_minima, resolutions) =
        resolve_downstream(&elevations, &terminals, &lake_candidates, &adjacency);
    let watersheds = resolve_watersheds(&downstream, &resolutions);
    let flow_accumulation = resolve_flow_accumulation(&downstream, &terminals, macro_map, patch);
    let terminal_indices = resolve_terminal_indices(&downstream);
    let lake_policies =
        resolve_lake_terminal_policies(&terminal_indices, &lake_candidates, &resolutions, config);
    let selected = select_river_paths(
        &downstream,
        &downstream_edges,
        &flow_accumulation,
        &elevations,
        &terminals,
        &terminal_indices,
        &lake_policies,
        config,
    );
    let selected_flow_accumulation =
        resolve_selected_flow_accumulation(&flow_accumulation, &terminal_indices, &lake_policies);
    let node_kinds = resolve_node_kinds(&selected, &downstream, &terminals, &resolutions);
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
    }
}

#[derive(Debug, Clone, Copy)]
struct CornerNeighbor {
    index: usize,
    edge: VoronoiEdgeId,
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
        || corner.signed_macro_elevation <= 0.0
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

        if let Some(path) = spill_path_to_outlet(index, elevations, terminals, adjacency) {
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
            resolutions[index] = if lake_candidates[index] {
                GraphLocalMinimumResolution::Lake
            } else {
                GraphLocalMinimumResolution::Sink
            };
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
    max_incoming_chains: usize,
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
        let area_scale = (area_units as f32).sqrt();
        let max_incoming_chains = area_scale.ceil().max(1.0) as usize;
        let discharge_cap =
            config.lake_discharge_cap_floor + area_scale * config.lake_discharge_cap_per_area;
        policies[index] = Some(LakeTerminalPolicy {
            max_incoming_chains,
            discharge_cap,
        });
    }

    policies
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
    terminal_indices: &[usize],
    lake_policies: &[Option<LakeTerminalPolicy>],
    config: HydrologyConfig,
) -> Vec<bool> {
    let mut selected = vec![false; downstream.len()];
    let mut selected_lake_chains = vec![0_usize; downstream.len()];
    let mut candidates = flow
        .iter()
        .enumerate()
        .filter_map(|(index, &amount)| {
            let terminal = terminal_indices[index];
            let threshold = if lake_policies[terminal].is_some() {
                config.river_flow_threshold * config.lake_river_flow_threshold_multiplier
            } else {
                config.river_flow_threshold
            };
            (amount >= threshold
                && elevations[index] >= config.headwater_elevation
                && downstream_edges[index].is_some()
                && !terminals[index])
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
        if let Some(policy) = lake_policies[terminal] {
            if selected_lake_chains[terminal] >= policy.max_incoming_chains {
                continue;
            }
            selected_lake_chains[terminal] += 1;
        }

        let mut current = start;
        let mut guard = 0;
        while let Some(next) = downstream[current] {
            if downstream_edges[current].is_some() {
                selected[current] = true;
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
    }

    selected
}

fn resolve_selected_flow_accumulation(
    raw_flow: &[f32],
    terminal_indices: &[usize],
    lake_policies: &[Option<LakeTerminalPolicy>],
) -> Vec<f32> {
    raw_flow
        .par_iter()
        .enumerate()
        .map(|(index, &flow)| {
            let terminal = terminal_indices[index];
            if let Some(policy) = lake_policies[terminal] {
                flow.min(policy.discharge_cap)
            } else {
                flow
            }
        })
        .collect()
}

fn resolve_node_kinds(
    selected: &[bool],
    downstream: &[Option<usize>],
    terminals: &[bool],
    resolutions: &[GraphLocalMinimumResolution],
) -> Vec<GraphDrainageNodeKind> {
    let mut incoming_selected = vec![0_u32; selected.len()];

    for (index, is_selected) in selected.iter().copied().enumerate() {
        if !is_selected {
            continue;
        }
        if let Some(target) = downstream[index] {
            incoming_selected[target] = incoming_selected[target].saturating_add(1);
        }
    }

    (0..selected.len())
        .into_par_iter()
        .map(|index| {
            if (terminals[index] || resolutions[index] == GraphLocalMinimumResolution::OceanOutlet)
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
    use crate::world::generation::macro_map::{MacroMapConfig, generate_macro_map};
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
    fn lake_terminal_policy_limits_selected_incoming_chains_by_area() {
        let downstream = vec![Some(3), Some(3), Some(3), None];
        let downstream_edges = vec![
            Some(VoronoiEdgeId(10)),
            Some(VoronoiEdgeId(11)),
            Some(VoronoiEdgeId(12)),
            None,
        ];
        let flow = vec![42.0, 40.0, 38.0, 120.0];
        let elevations = vec![0.8, 0.7, 0.6, 0.1];
        let terminals = vec![false, false, false, false];
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

        let selected = select_river_paths(
            &downstream,
            &downstream_edges,
            &flow,
            &elevations,
            &terminals,
            &terminal_indices,
            &lake_policies,
            config,
        );

        let incoming_to_lake = selected[..3].iter().filter(|&&selected| selected).count();
        assert_eq!(
            incoming_to_lake, 1,
            "a tiny lake should accept only one selected incoming river chain"
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

        let selected_flow =
            resolve_selected_flow_accumulation(&raw_flow, &terminal_indices, &lake_policies);
        let policy = lake_policies[3].expect("lake terminal should have policy");

        assert!(policy.discharge_cap < raw_flow[2]);
        assert_eq!(selected_flow[2], policy.discharge_cap);
        assert_eq!(
            raw_flow[2], 220.0,
            "raw hydrology ledger should remain unchanged"
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
}
