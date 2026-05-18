use rayon::prelude::*;
use std::collections::{HashMap, VecDeque};

use super::super::graph::{VoronoiCornerId, VoronoiEdgeId};
use super::super::macro_map::{MacroEdge, MacroSurfaceKind};
use super::routing::CornerNeighbor;
use super::types::{
    GraphDrainageNodeKind, GraphHydrologyTopologyStats, GraphLocalMinimumResolution,
    HydrologyConfig, LakeContactTopology, LakeTerminalPolicy,
};

pub(super) fn extend_selected_river_mouths_one_ocean_edge(
    selected: &mut [bool],
    downstream: &mut [Option<usize>],
    downstream_edges: &mut [Option<VoronoiEdgeId>],
    terminals: &[bool],
    resolutions: &[GraphLocalMinimumResolution],
    corner_surface_kinds: &[Option<MacroSurfaceKind>],
    lake_candidates: &[bool],
    adjacency: &[Vec<CornerNeighbor>],
    edge_map: &HashMap<VoronoiEdgeId, MacroEdge>,
) -> usize {
    let mut incoming_selected = vec![0_u32; selected.len()];
    let mut outgoing_selected = vec![0_u32; selected.len()];
    for (index, is_selected) in selected.iter().copied().enumerate() {
        if !is_selected {
            continue;
        }
        outgoing_selected[index] = outgoing_selected[index].saturating_add(1);
        if let Some(target) = downstream[index] {
            incoming_selected[target] = incoming_selected[target].saturating_add(1);
        }
    }

    let mut extended = 0usize;
    for index in 0..selected.len() {
        if incoming_selected[index] == 0
            || outgoing_selected[index] > 0
            || !(terminals[index] || resolutions[index] == GraphLocalMinimumResolution::OceanOutlet)
            || lake_candidates[index]
        {
            continue;
        }

        let Some(candidate) = adjacency[index]
            .iter()
            .copied()
            .filter(|neighbor| {
                !lake_candidates[neighbor.index]
                    && !edge_map
                        .get(&neighbor.edge)
                        .is_some_and(|edge| edge.lake_class.excludes_selected_river())
                    && corner_surface_kinds
                        .get(neighbor.index)
                        .copied()
                        .flatten()
                        .is_some_and(MacroSurfaceKind::is_ocean_owned)
            })
            .min_by(|left, right| {
                left.index
                    .cmp(&right.index)
                    .then_with(|| left.edge.0.cmp(&right.edge.0))
            })
        else {
            continue;
        };

        downstream[index] = Some(candidate.index);
        downstream_edges[index] = Some(candidate.edge);
        selected[index] = true;
        extended += 1;
    }

    extended
}

pub(super) fn resolve_lake_contact_topology(
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

pub(super) fn lake_components(
    lake_candidates: &[bool],
    adjacency: &[Vec<CornerNeighbor>],
) -> Vec<Vec<usize>> {
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

pub(super) fn lake_hop_distances(
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

pub(super) fn choose_lake_outlet(
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

pub(super) fn enforce_lake_contact_topology(
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

pub(super) fn remove_invalid_terminal_intersections(
    selected: &mut [bool],
    downstream: &[Option<usize>],
    flow: &[f32],
    lake_candidates: &[bool],
    terminals: &[bool],
    resolutions: &[GraphLocalMinimumResolution],
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
        let is_explicit_terminal = terminals[corner]
            || matches!(
                resolutions[corner],
                GraphLocalMinimumResolution::OceanOutlet
                    | GraphLocalMinimumResolution::OutletCarve
                    | GraphLocalMinimumResolution::Lake
                    | GraphLocalMinimumResolution::Sink
            );
        if sources.len() <= 1
            || has_outgoing[corner]
            || lake_candidates[corner]
            || is_explicit_terminal
        {
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

pub(super) fn prune_multi_incoming_selected_branches(
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

        let Some((corner, sources)) = incoming
            .iter()
            .enumerate()
            .find(|(corner, sources)| sources.len() > selected_incoming_limit(*corner, selected))
        else {
            break;
        };
        let keep = strongest_sources(sources, selected_incoming_limit(corner, selected), flow);
        for source in sources.iter().copied() {
            if keep.contains(&source) {
                continue;
            }
            if selected[source] {
                selected[source] = false;
                pruned += 1;
            }
        }
    }

    pruned
}

pub(super) fn prune_duplicate_corner_outgoing_selected_branches(
    selected: &mut [bool],
    downstream: &[Option<usize>],
    flow: &[f32],
    corner_ids: &[VoronoiCornerId],
) -> usize {
    let mut outgoing_by_corner = HashMap::<VoronoiCornerId, Vec<usize>>::new();

    for (index, is_selected) in selected.iter().copied().enumerate() {
        if !is_selected || downstream.get(index).copied().flatten().is_none() {
            continue;
        }
        let Some(corner) = corner_ids.get(index).copied() else {
            continue;
        };
        outgoing_by_corner.entry(corner).or_default().push(index);
    }

    let mut pruned = 0;
    for sources in outgoing_by_corner.values() {
        if sources.len() <= 1 {
            continue;
        }
        let keep = strongest_sources(sources, 1, flow)
            .into_iter()
            .next()
            .expect("duplicate outgoing corner should keep one source");
        for source in sources {
            if *source == keep || !selected[*source] {
                continue;
            }
            selected[*source] = false;
            pruned += 1;
        }
    }

    pruned
}

fn selected_incoming_limit(corner: usize, selected: &[bool]) -> usize {
    if selected.get(corner).copied().unwrap_or(false) {
        2
    } else {
        1
    }
}

fn strongest_sources(sources: &[usize], limit: usize, flow: &[f32]) -> Vec<usize> {
    let mut ordered = sources.to_vec();
    ordered.sort_by(|&left, &right| {
        flow[right]
            .total_cmp(&flow[left])
            .then_with(|| left.cmp(&right))
    });
    ordered.truncate(limit.max(1));
    ordered
}

pub(super) fn remove_repeated_lake_contact_chains(
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

pub(super) fn prune_disconnected_selected_fragments(
    selected: &mut [bool],
    downstream: &[Option<usize>],
    terminals: &[bool],
    resolutions: &[GraphLocalMinimumResolution],
    lake_topology: &LakeContactTopology,
) -> usize {
    let mut incoming = vec![Vec::<usize>::new(); selected.len()];
    let mut keep = vec![false; selected.len()];
    let mut queue = VecDeque::new();

    for (source, is_selected) in selected.iter().copied().enumerate() {
        if !is_selected {
            continue;
        }
        let Some(target) = downstream[source] else {
            continue;
        };
        incoming[target].push(source);
        if selected_ocean_terminal_is_valid(target, terminals, resolutions)
            || selected_lake_inlet_terminal_is_valid(target, downstream, lake_topology)
        {
            keep[source] = true;
            queue.push_back(source);
        }
    }

    while let Some(reachable) = queue.pop_front() {
        for &upstream in &incoming[reachable] {
            if keep[upstream] {
                continue;
            }
            keep[upstream] = true;
            queue.push_back(upstream);
        }
    }

    let mut pruned = 0;
    for (index, is_selected) in selected.iter_mut().enumerate() {
        if *is_selected && !keep[index] {
            *is_selected = false;
            pruned += 1;
        }
    }

    pruned
}

pub(super) fn selected_ocean_terminal_is_valid(
    index: usize,
    terminals: &[bool],
    resolutions: &[GraphLocalMinimumResolution],
) -> bool {
    terminals.get(index).copied().unwrap_or(false)
        || matches!(
            resolutions.get(index).copied(),
            Some(GraphLocalMinimumResolution::OceanOutlet)
        )
}

pub(super) fn selected_lake_inlet_terminal_is_valid(
    index: usize,
    downstream: &[Option<usize>],
    lake_topology: &LakeContactTopology,
) -> bool {
    lake_topology
        .inlet_land_vertices
        .get(index)
        .copied()
        .unwrap_or(false)
        && downstream
            .get(index)
            .copied()
            .flatten()
            .is_some_and(|lake| {
                lake_topology
                    .component_by_corner
                    .get(lake)
                    .copied()
                    .flatten()
                    .is_some()
            })
}

pub(super) fn resolve_node_kinds(
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

pub(super) fn resolve_topology_stats(
    selected: &[bool],
    downstream: &[Option<usize>],
    lake_candidates: &[bool],
    resolutions: &[GraphLocalMinimumResolution],
    downstream_edges: &[Option<VoronoiEdgeId>],
    edge_map: &HashMap<VoronoiEdgeId, MacroEdge>,
    node_kinds: &[GraphDrainageNodeKind],
    duplicate_trunk_pruned_count: usize,
    repeated_lake_contact_pruned_count: usize,
    disconnected_river_fragment_pruned_count: usize,
) -> GraphHydrologyTopologyStats {
    let mut stats = GraphHydrologyTopologyStats::default();
    stats.duplicate_trunk_pruned_count = duplicate_trunk_pruned_count;
    stats.repeated_lake_contact_pruned_count = repeated_lake_contact_pruned_count;
    stats.disconnected_river_fragment_pruned_count = disconnected_river_fragment_pruned_count;
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
        let incoming_limit = if outgoing[index] > 0 {
            2
        } else if matches!(
            resolutions[index],
            GraphLocalMinimumResolution::OceanOutlet
                | GraphLocalMinimumResolution::OutletCarve
                | GraphLocalMinimumResolution::Lake
                | GraphLocalMinimumResolution::Sink
        ) {
            2
        } else {
            1
        };
        if incoming[index] <= incoming_limit {
            continue;
        }
        let is_explicit_terminal = matches!(
            resolutions[index],
            GraphLocalMinimumResolution::OceanOutlet
                | GraphLocalMinimumResolution::OutletCarve
                | GraphLocalMinimumResolution::Lake
                | GraphLocalMinimumResolution::Sink
        );
        if !is_explicit_terminal || incoming[index] > 2 {
            stats.invalid_river_intersection_count +=
                incoming[index].saturating_sub(incoming_limit) as usize;
        }
        if lake_candidates[index] && incoming[index] > 1 {
            stats.invalid_river_intersection_count +=
                incoming[index].saturating_sub(incoming_limit) as usize;
        }
        if !is_explicit_terminal {
            stats.ambiguous_shared_corner_count +=
                incoming[index].saturating_sub(incoming_limit) as usize;
        }
    }

    stats
}

pub(super) fn is_lake_edge(
    edge: VoronoiEdgeId,
    edge_map: &HashMap<VoronoiEdgeId, MacroEdge>,
) -> bool {
    edge_map
        .get(&edge)
        .is_some_and(|edge| edge.lake_class.excludes_selected_river())
}
