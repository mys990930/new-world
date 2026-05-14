use rayon::prelude::*;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, VecDeque};

use super::super::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiGraphPatch};
use super::super::macro_map::{GraphMacroMap, MacroCorner, MacroSurfaceKind};
use super::types::{GraphLocalMinimumResolution, WatershedId, splitmix64};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CornerNeighbor {
    pub(super) index: usize,
    pub(super) edge: VoronoiEdgeId,
}

pub(super) fn corner_adjacency(
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

pub(super) fn is_terminal_outlet(corner: MacroCorner) -> bool {
    corner.surface_kind.is_ocean_owned()
        || matches!(corner.surface_kind, MacroSurfaceKind::CoastOcean)
}

pub(super) fn resolve_downstream(
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

pub(super) fn steepest_lower_neighbor(
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

pub(super) fn spill_path_to_outlet(
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

pub(super) fn reconstruct_path(start: usize, end: usize, previous: &[Option<usize>]) -> Vec<usize> {
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

pub(super) fn edge_between(
    from: usize,
    to: usize,
    adjacency: &[Vec<CornerNeighbor>],
) -> Option<VoronoiEdgeId> {
    adjacency[from]
        .iter()
        .find(|neighbor| neighbor.index == to)
        .map(|neighbor| neighbor.edge)
}

pub(super) fn break_remaining_cycles(
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

pub(super) fn resolve_watersheds(
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

pub(super) fn downstream_outlet(start: usize, downstream: &[Option<usize>]) -> usize {
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

pub(super) fn resolve_terminal_indices(downstream: &[Option<usize>]) -> Vec<usize> {
    (0..downstream.len())
        .into_par_iter()
        .map(|index| downstream_outlet(index, downstream))
        .collect()
}

pub(super) fn resolve_first_downstream_lakes(
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

pub(super) fn resolve_flow_accumulation(
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
