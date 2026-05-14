use std::collections::{HashMap, VecDeque};

use super::super::graph::VoronoiEdgeId;
use super::super::macro_map::MacroEdge;
use super::routing::CornerNeighbor;
use super::topology::{
    enforce_lake_contact_topology, is_lake_edge, prune_disconnected_selected_fragments,
    prune_multi_incoming_selected_branches, remove_invalid_terminal_intersections,
    remove_repeated_lake_contact_chains,
};
use super::types::{
    DEFAULT_HEADWATER_SOURCE_HYDRATION, DEFAULT_HEADWATER_SOURCE_SCORE,
    GraphLocalMinimumResolution, HydrologyConfig, LAKE_INLET_RIVER_THRESHOLD_CAP,
    LakeContactTopology, LakeTerminalPolicy,
};

pub(super) struct SelectedRiverPaths {
    pub(super) selected: Vec<bool>,
    pub(super) duplicate_trunk_pruned_count: usize,
    pub(super) repeated_lake_contact_pruned_count: usize,
    pub(super) disconnected_river_fragment_pruned_count: usize,
}

pub(super) fn resolve_lake_terminal_policies(
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

pub(super) fn resolve_lake_inlet_policies(
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

pub(super) fn lake_policy_for_area(
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

pub(super) fn lake_discharge_floor_for_area(area_units: f32, config: HydrologyConfig) -> f32 {
    (config.lake_discharge_cap_floor * 0.55 + area_units * config.lake_discharge_range_per_area)
        .min(config.lake_discharge_cap_ceiling * 0.55)
}

pub(super) fn lake_inlet_selection_threshold(
    area_units: f32,
    discharge_cap: f32,
    config: HydrologyConfig,
) -> f32 {
    let area_scale = (area_units / config.lake_area_units_per_chain).sqrt();
    let base_threshold = config
        .river_flow_threshold
        .min(LAKE_INLET_RIVER_THRESHOLD_CAP);
    (base_threshold * (3.0 + area_scale)).max(discharge_cap * 1.35)
}

pub(super) fn select_river_paths(
    downstream: &[Option<usize>],
    downstream_edges: &[Option<VoronoiEdgeId>],
    flow: &[f32],
    elevations: &[f32],
    source_hydration: &[f32],
    terminals: &[bool],
    lake_candidates: &[bool],
    resolutions: &[GraphLocalMinimumResolution],
    terminal_indices: &[usize],
    first_downstream_lakes: &[Option<usize>],
    lake_policies: &[Option<LakeTerminalPolicy>],
    lake_inlet_policies: &[Option<LakeTerminalPolicy>],
    lake_topology: &LakeContactTopology,
    adjacency: &[Vec<CornerNeighbor>],
    edge_map: &HashMap<VoronoiEdgeId, MacroEdge>,
    config: HydrologyConfig,
) -> SelectedRiverPaths {
    let mut selected = vec![false; downstream.len()];
    let mut selected_lake_chains = vec![0_usize; downstream.len()];
    let mut candidates = downstream
        .iter()
        .enumerate()
        .filter_map(|(index, _)| {
            let terminal = terminal_indices[index];
            let lake_policy = lake_policies[terminal].or_else(|| {
                first_downstream_lakes[index].and_then(|lake| lake_inlet_policies[lake])
            });
            let threshold = if let Some(policy) = lake_policy {
                policy.selection_threshold
            } else {
                config.river_flow_threshold
            };
            let source = headwater_source_candidate(
                index,
                downstream,
                downstream_edges,
                flow,
                elevations,
                source_hydration,
                terminals,
                lake_candidates,
                first_downstream_lakes,
                adjacency,
                edge_map,
                config,
                threshold,
            )?;
            Some(source)
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|&left, &right| {
        right
            .path_potential
            .total_cmp(&left.path_potential)
            .then_with(|| right.score.total_cmp(&left.score))
            .then_with(|| right.elevation.total_cmp(&left.elevation))
            .then_with(|| left.index.cmp(&right.index))
    });

    for candidate in candidates {
        let start = candidate.index;
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
    remove_invalid_terminal_intersections(
        &mut selected,
        downstream,
        flow,
        lake_candidates,
        terminals,
        resolutions,
    );
    let mut duplicate_trunk_pruned_count =
        prune_multi_incoming_selected_branches(&mut selected, downstream, flow);
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
    duplicate_trunk_pruned_count +=
        prune_multi_incoming_selected_branches(&mut selected, downstream, flow);
    let disconnected_river_fragment_pruned_count = prune_disconnected_selected_fragments(
        &mut selected,
        downstream,
        terminals,
        resolutions,
        lake_topology,
    );

    SelectedRiverPaths {
        selected,
        duplicate_trunk_pruned_count,
        repeated_lake_contact_pruned_count,
        disconnected_river_fragment_pruned_count,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct HeadwaterSourceCandidate {
    index: usize,
    path_potential: f32,
    score: f32,
    elevation: f32,
}

#[allow(clippy::too_many_arguments)]
fn headwater_source_candidate(
    index: usize,
    downstream: &[Option<usize>],
    downstream_edges: &[Option<VoronoiEdgeId>],
    flow: &[f32],
    elevations: &[f32],
    source_hydration: &[f32],
    terminals: &[bool],
    lake_candidates: &[bool],
    first_downstream_lakes: &[Option<usize>],
    adjacency: &[Vec<CornerNeighbor>],
    edge_map: &HashMap<VoronoiEdgeId, MacroEdge>,
    config: HydrologyConfig,
    threshold: f32,
) -> Option<HeadwaterSourceCandidate> {
    if terminals[index]
        || lake_candidates[index]
        || elevations[index] < config.headwater_elevation
        || downstream_edges[index].is_none_or(|edge| is_lake_edge(edge, edge_map))
    {
        return None;
    }

    let ridge_context = source_ridge_context(index, adjacency, edge_map);
    let local_peak =
        is_headwater_local_peak(index, elevations, terminals, lake_candidates, adjacency);
    let near_local_peak = is_headwater_near_local_peak(
        index,
        elevations,
        terminals,
        lake_candidates,
        adjacency,
        0.08,
    );
    let high_context =
        elevations[index] >= (config.headwater_elevation + 0.28).max(0.38) || ridge_context >= 0.28;
    let hydration = source_hydration.get(index).copied().unwrap_or(0.0);
    let hydrated_source_context = hydration >= 0.44 && near_local_peak;
    if !local_peak && !high_context && !hydrated_source_context {
        return None;
    }

    let elevation_bonus = ((elevations[index] - config.headwater_elevation) / 0.55).clamp(0.0, 1.0);
    let score = hydration * 0.52 + elevation_bonus * 0.30 + ridge_context * 0.30;
    if hydration < DEFAULT_HEADWATER_SOURCE_HYDRATION && score < DEFAULT_HEADWATER_SOURCE_SCORE {
        return None;
    }

    let path_potential = downstream_path_potential(
        index,
        downstream,
        flow,
        terminals,
        lake_candidates,
        first_downstream_lakes,
    );
    if path_potential < threshold {
        return None;
    }

    Some(HeadwaterSourceCandidate {
        index,
        path_potential,
        score,
        elevation: elevations[index],
    })
}

pub(super) fn is_headwater_local_peak(
    index: usize,
    elevations: &[f32],
    terminals: &[bool],
    lake_candidates: &[bool],
    adjacency: &[Vec<CornerNeighbor>],
) -> bool {
    adjacency[index]
        .iter()
        .filter(|neighbor| !terminals[neighbor.index] && !lake_candidates[neighbor.index])
        .all(|neighbor| elevations[index] >= elevations[neighbor.index] - 0.001)
}

pub(super) fn is_headwater_near_local_peak(
    index: usize,
    elevations: &[f32],
    terminals: &[bool],
    lake_candidates: &[bool],
    adjacency: &[Vec<CornerNeighbor>],
    tolerance: f32,
) -> bool {
    adjacency[index]
        .iter()
        .filter(|neighbor| !terminals[neighbor.index] && !lake_candidates[neighbor.index])
        .all(|neighbor| elevations[index] + tolerance >= elevations[neighbor.index])
}

pub(super) fn source_ridge_context(
    index: usize,
    adjacency: &[Vec<CornerNeighbor>],
    edge_map: &HashMap<VoronoiEdgeId, MacroEdge>,
) -> f32 {
    adjacency[index]
        .iter()
        .filter_map(|neighbor| edge_map.get(&neighbor.edge))
        .map(|edge| {
            let guide = edge.guide;
            let explicit_ridge = if guide.is_ridge_candidate { 0.30 } else { 0.0 };
            guide
                .mountainness
                .max(guide.ridgeness)
                .max(guide.drainage_divide_potential)
                .max(explicit_ridge)
        })
        .fold(0.0, f32::max)
}

pub(super) fn downstream_path_potential(
    start: usize,
    downstream: &[Option<usize>],
    flow: &[f32],
    terminals: &[bool],
    lake_candidates: &[bool],
    first_downstream_lakes: &[Option<usize>],
) -> f32 {
    let first_lake = first_downstream_lakes[start];
    let mut current = start;
    let mut best = 0.0_f32;
    let mut guard = 0;

    loop {
        if terminals[current] || lake_candidates[current] {
            break;
        }
        best = best.max(flow[current]);
        let Some(next) = downstream[current] else {
            break;
        };
        if first_lake.is_some_and(|lake| next == lake) || terminals[next] {
            best = best.max(flow[current]);
            break;
        }
        current = next;
        guard += 1;
        if guard > downstream.len() {
            break;
        }
    }

    best
}
