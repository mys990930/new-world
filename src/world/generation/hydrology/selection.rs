use std::collections::{HashMap, HashSet, VecDeque};

use super::super::graph::{VoronoiEdgeId, WorldPlanePoint};
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
    corner_positions: Option<&[WorldPlanePoint]>,
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

    sort_headwater_candidates(&mut candidates);

    let mut accepted_source_paths = Vec::new();
    for candidate in candidates {
        let start = candidate.index;
        let terminal = terminal_indices[start];
        let first_lake = first_downstream_lakes[start];
        let lake_policy = lake_policies[terminal]
            .or_else(|| first_lake.and_then(|lake| lake_inlet_policies[lake]));

        let chain =
            selected_chain_from_source(start, downstream, downstream_edges, terminals, first_lake);
        if chain_intersects_selected_path(&chain, &selected, downstream) {
            continue;
        }
        let accepted_candidate = SelectedSourcePathCandidate {
            source: candidate,
            path: chain.clone(),
            merge_index: None,
        };
        if selected_source_path_conflicts(
            &accepted_candidate,
            &accepted_source_paths,
            corner_positions,
            config,
        ) {
            continue;
        }

        if let Some(policy) = lake_policy {
            if selected_lake_chains[policy.component_root] >= policy.max_incoming_chains {
                continue;
            }
            selected_lake_chains[policy.component_root] += 1;
        }

        for index in chain {
            selected[index] = true;
        }
        accepted_source_paths.push(accepted_candidate);
    }

    let mainstem_selected = selected.clone();
    let mut incoming_selected = selected_incoming_counts(&selected, downstream);
    let mut tributary_candidates = downstream
        .iter()
        .enumerate()
        .filter_map(|(index, _)| {
            explicit_tributary_source_candidate(
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
                &mainstem_selected,
                config,
            )
        })
        .collect::<Vec<_>>();

    tributary_candidates.sort_by(|left, right| {
        right
            .source
            .score
            .total_cmp(&left.source.score)
            .then_with(|| {
                right
                    .source
                    .path_potential
                    .total_cmp(&left.source.path_potential)
            })
            .then_with(|| left.path.len().cmp(&right.path.len()))
            .then_with(|| right.source.elevation.total_cmp(&left.source.elevation))
            .then_with(|| left.source.index.cmp(&right.source.index))
    });

    for candidate in tributary_candidates {
        if !tributary_path_can_merge(
            &candidate,
            &selected,
            &mainstem_selected,
            &incoming_selected,
            downstream,
            downstream_edges,
            edge_map,
        ) {
            continue;
        }
        let accepted_candidate = SelectedSourcePathCandidate {
            source: candidate.source,
            path: candidate.path.clone(),
            merge_index: Some(candidate.merge_index),
        };
        if selected_source_path_conflicts(
            &accepted_candidate,
            &accepted_source_paths,
            corner_positions,
            config,
        ) {
            continue;
        }
        for index in &candidate.path {
            selected[*index] = true;
            if let Some(target) = downstream[*index] {
                incoming_selected[target] = incoming_selected[target].saturating_add(1);
            }
        }
        accepted_source_paths.push(accepted_candidate);
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
    duplicate_trunk_pruned_count +=
        prune_spatial_confluence_splits(&mut selected, downstream, flow, corner_positions);
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

fn prune_spatial_confluence_splits(
    selected: &mut [bool],
    downstream: &[Option<usize>],
    flow: &[f32],
    corner_positions: Option<&[WorldPlanePoint]>,
) -> usize {
    let Some(positions) = corner_positions else {
        return 0;
    };

    let mut groups = HashMap::<(i32, i32), Vec<usize>>::new();
    for (index, position) in positions.iter().copied().enumerate() {
        groups
            .entry(spatial_corner_key(position))
            .or_default()
            .push(index);
    }

    let mut pruned = 0_usize;
    for group in groups.values().filter(|group| group.len() > 1) {
        let members = group.iter().copied().collect::<HashSet<_>>();
        let mut incoming = Vec::new();
        let mut outgoing = Vec::new();

        for (source, is_selected) in selected.iter().copied().enumerate() {
            if !is_selected {
                continue;
            }
            let Some(target) = downstream[source] else {
                continue;
            };
            let source_in_group = members.contains(&source);
            let target_in_group = members.contains(&target);
            if !source_in_group && target_in_group {
                incoming.push(source);
            } else if source_in_group && !target_in_group {
                outgoing.push(source);
            }
        }

        if incoming.is_empty() || outgoing.len() <= 1 {
            continue;
        }

        let keep = outgoing
            .iter()
            .copied()
            .max_by(|&left, &right| {
                flow[left]
                    .total_cmp(&flow[right])
                    .then_with(|| right.cmp(&left))
            })
            .expect("non-empty outgoing list should have a keep candidate");
        for source in outgoing {
            if source != keep && selected[source] {
                selected[source] = false;
                pruned += 1;
            }
        }
    }

    pruned
}

fn spatial_corner_key(position: WorldPlanePoint) -> (i32, i32) {
    const EPSILON_BLOCKS: f32 = 1.0;
    (
        (position.x / EPSILON_BLOCKS).round() as i32,
        (position.z / EPSILON_BLOCKS).round() as i32,
    )
}

fn selected_source_path_conflicts(
    candidate: &SelectedSourcePathCandidate,
    accepted: &[SelectedSourcePathCandidate],
    corner_positions: Option<&[WorldPlanePoint]>,
    config: HydrologyConfig,
) -> bool {
    if accepted.is_empty() {
        return false;
    }
    let Some(positions) = corner_positions else {
        return false;
    };

    accepted.iter().any(|other| {
        shared_tributary_merge_conflicts(candidate, other, positions, config)
            || source_spacing_conflicts(candidate, other, positions, config)
            || early_parallel_path_conflicts(candidate, other, positions, config)
    })
}

fn source_spacing_conflicts(
    candidate: &SelectedSourcePathCandidate,
    other: &SelectedSourcePathCandidate,
    positions: &[WorldPlanePoint],
    config: HydrologyConfig,
) -> bool {
    let min_spacing = config.tributary_source_min_spacing_blocks;
    min_spacing > 0.0
        && squared_corner_distance(candidate.source.index, other.source.index, positions)
            .is_some_and(|distance| distance < min_spacing * min_spacing)
}

fn early_parallel_path_conflicts(
    candidate: &SelectedSourcePathCandidate,
    other: &SelectedSourcePathCandidate,
    positions: &[WorldPlanePoint],
    config: HydrologyConfig,
) -> bool {
    let min_spacing = config.tributary_parallel_path_min_spacing_blocks;
    if min_spacing <= 0.0 {
        return false;
    }

    let compare_edges = config.tributary_parallel_path_compare_edges as usize;
    let close_pairs = candidate
        .path
        .iter()
        .take(compare_edges)
        .filter(|&&left| {
            other.path.iter().take(compare_edges).any(|&right| {
                squared_corner_distance(left, right, positions)
                    .is_some_and(|distance| distance < min_spacing * min_spacing)
            })
        })
        .count();

    close_pairs >= 2
}

fn shared_tributary_merge_conflicts(
    candidate: &SelectedSourcePathCandidate,
    other: &SelectedSourcePathCandidate,
    positions: &[WorldPlanePoint],
    config: HydrologyConfig,
) -> bool {
    let (Some(candidate_merge), Some(other_merge)) = (candidate.merge_index, other.merge_index)
    else {
        return false;
    };
    if candidate_merge == other_merge {
        return true;
    }

    let min_spacing = config.tributary_parallel_path_min_spacing_blocks;
    min_spacing > 0.0
        && squared_corner_distance(candidate_merge, other_merge, positions)
            .is_some_and(|distance| distance < min_spacing * min_spacing)
}

fn squared_corner_distance(
    left: usize,
    right: usize,
    positions: &[WorldPlanePoint],
) -> Option<f32> {
    let left = positions.get(left).copied()?;
    let right = positions.get(right).copied()?;
    let dx = left.x - right.x;
    let dz = left.z - right.z;
    Some(dx * dx + dz * dz)
}

fn sort_headwater_candidates(candidates: &mut [HeadwaterSourceCandidate]) {
    candidates.sort_by(|&left, &right| {
        right
            .path_potential
            .total_cmp(&left.path_potential)
            .then_with(|| right.score.total_cmp(&left.score))
            .then_with(|| right.elevation.total_cmp(&left.elevation))
            .then_with(|| left.index.cmp(&right.index))
    });
}

fn selected_chain_from_source(
    start: usize,
    downstream: &[Option<usize>],
    downstream_edges: &[Option<VoronoiEdgeId>],
    terminals: &[bool],
    first_lake: Option<usize>,
) -> Vec<usize> {
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

    chain
}

fn chain_intersects_selected_path(
    chain: &[usize],
    selected: &[bool],
    downstream: &[Option<usize>],
) -> bool {
    chain.iter().copied().any(|source| {
        selected.get(source).copied().unwrap_or(false)
            || downstream[source]
                .is_some_and(|target| selected.get(target).copied().unwrap_or(false))
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct HeadwaterSourceCandidate {
    index: usize,
    path_potential: f32,
    score: f32,
    elevation: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct TributarySourceCandidate {
    source: HeadwaterSourceCandidate,
    path: Vec<usize>,
    merge_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct SelectedSourcePathCandidate {
    source: HeadwaterSourceCandidate,
    path: Vec<usize>,
    merge_index: Option<usize>,
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

#[allow(clippy::too_many_arguments)]
fn explicit_tributary_source_candidate(
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
    mainstem_selected: &[bool],
    config: HydrologyConfig,
) -> Option<TributarySourceCandidate> {
    if mainstem_selected.get(index).copied().unwrap_or(false) {
        return None;
    }
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
        0.0,
    )?;
    let hydration = source_hydration.get(index).copied().unwrap_or(0.0);
    if hydration < config.tributary_source_hydration
        || source.score < config.tributary_source_threshold
    {
        return None;
    }
    let (path, merge_index) = tributary_path_to_mainstem(
        index,
        downstream,
        downstream_edges,
        terminals,
        lake_candidates,
        edge_map,
        mainstem_selected,
        config.tributary_max_path_edges as usize,
    )?;

    Some(TributarySourceCandidate {
        source,
        path,
        merge_index,
    })
}

fn tributary_path_to_mainstem(
    start: usize,
    downstream: &[Option<usize>],
    downstream_edges: &[Option<VoronoiEdgeId>],
    terminals: &[bool],
    lake_candidates: &[bool],
    edge_map: &HashMap<VoronoiEdgeId, MacroEdge>,
    mainstem_selected: &[bool],
    max_path_edges: usize,
) -> Option<(Vec<usize>, usize)> {
    let mut path = Vec::new();
    let mut current = start;
    let mut visited = HashSet::new();

    for _ in 0..max_path_edges {
        if !visited.insert(current)
            || terminals.get(current).copied().unwrap_or(false)
            || lake_candidates.get(current).copied().unwrap_or(false)
            || mainstem_selected.get(current).copied().unwrap_or(false)
        {
            return None;
        }
        let edge = downstream_edges.get(current).copied().flatten()?;
        if is_lake_edge(edge, edge_map) {
            return None;
        }
        let next = downstream.get(current).copied().flatten()?;
        path.push(current);

        if mainstem_selected.get(next).copied().unwrap_or(false) {
            return Some((path, next));
        }
        if terminals.get(next).copied().unwrap_or(false)
            || lake_candidates.get(next).copied().unwrap_or(false)
        {
            return None;
        }

        current = next;
    }

    None
}

fn selected_incoming_counts(selected: &[bool], downstream: &[Option<usize>]) -> Vec<u32> {
    let mut incoming = vec![0_u32; selected.len()];
    for (index, is_selected) in selected.iter().copied().enumerate() {
        if !is_selected {
            continue;
        }
        if let Some(target) = downstream[index] {
            incoming[target] = incoming[target].saturating_add(1);
        }
    }
    incoming
}

fn tributary_path_can_merge(
    candidate: &TributarySourceCandidate,
    selected: &[bool],
    mainstem_selected: &[bool],
    incoming_selected: &[u32],
    downstream: &[Option<usize>],
    downstream_edges: &[Option<VoronoiEdgeId>],
    edge_map: &HashMap<VoronoiEdgeId, MacroEdge>,
) -> bool {
    if candidate.path.is_empty()
        || !mainstem_selected
            .get(candidate.merge_index)
            .copied()
            .unwrap_or(false)
        || incoming_selected
            .get(candidate.merge_index)
            .copied()
            .unwrap_or(0)
            >= 2
    {
        return false;
    }

    for &source in &candidate.path {
        if selected.get(source).copied().unwrap_or(false) {
            return false;
        }
        let Some(edge) = downstream_edges.get(source).copied().flatten() else {
            return false;
        };
        if is_lake_edge(edge, edge_map) {
            return false;
        }
        let Some(target) = downstream.get(source).copied().flatten() else {
            return false;
        };
        if target == candidate.merge_index {
            continue;
        }
        if selected.get(target).copied().unwrap_or(false)
            || incoming_selected.get(target).copied().unwrap_or(0) > 0
        {
            return false;
        }
    }

    true
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
