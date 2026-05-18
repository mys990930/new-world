use rayon::prelude::*;

use super::super::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiGraphPatch, WorldPlanePoint};
use super::types::{
    splitmix64, GraphDrainageNode, GraphDrainageNodeId, GraphDrainageNodeKind, GraphHydrologyRole,
    GraphRiverSegment, GraphRiverSegmentId, LakeContactTopology, WatershedId,
};

pub(super) fn resolve_selected_flow_accumulation(
    raw_flow: &[f32],
    selected: &[bool],
    downstream: &[Option<usize>],
    lake_topology: &LakeContactTopology,
    node_kinds: &[GraphDrainageNodeKind],
) -> Vec<f32> {
    let mut selected_flow = raw_flow.to_vec();

    enforce_monotone_selected_display_flow(&mut selected_flow, selected, downstream, node_kinds);
    propagate_lake_system_display_flow(
        &mut selected_flow,
        selected,
        downstream,
        lake_topology,
        node_kinds,
    );
    enforce_monotone_selected_display_flow(&mut selected_flow, selected, downstream, node_kinds);
    selected_flow
}

pub(super) fn propagate_lake_system_display_flow(
    selected_flow: &mut [f32],
    selected: &[bool],
    downstream: &[Option<usize>],
    lake_topology: &LakeContactTopology,
    node_kinds: &[GraphDrainageNodeKind],
) {
    let mut component_inlet_flow = vec![0.0_f32; selected.len()];

    for (source, is_selected) in selected.iter().copied().enumerate() {
        if !is_selected {
            continue;
        }
        let Some(target) = downstream[source] else {
            continue;
        };
        if node_kinds
            .get(target)
            .copied()
            .is_some_and(|kind| kind == GraphDrainageNodeKind::LakeInlet)
        {
            let Some(lake_corner) = downstream[target] else {
                continue;
            };
            if let Some(component) = lake_topology.component_by_corner[lake_corner] {
                component_inlet_flow[component] =
                    component_inlet_flow[component].max(selected_flow[source]);
            }
        }
    }

    for (source, is_selected) in selected.iter().copied().enumerate() {
        if !is_selected {
            continue;
        }
        if !node_kinds
            .get(source)
            .copied()
            .is_some_and(|kind| kind == GraphDrainageNodeKind::LakeOutlet)
        {
            continue;
        }
        let Some(component) = lake_topology.contact_component_by_land_corner[source] else {
            continue;
        };
        selected_flow[source] = selected_flow[source].max(component_inlet_flow[component]);
    }
}

pub(super) fn enforce_monotone_selected_display_flow(
    selected_flow: &mut [f32],
    selected: &[bool],
    downstream: &[Option<usize>],
    node_kinds: &[GraphDrainageNodeKind],
) {
    let mut incoming = vec![0_u32; selected.len()];
    for (index, is_selected) in selected.iter().copied().enumerate() {
        if !is_selected {
            continue;
        }
        if let Some(target) = downstream[index] {
            incoming[target] = incoming[target].saturating_add(1);
        }
    }

    let mut visited = vec![false; selected.len()];
    for index in 0..selected.len() {
        if !selected[index] || incoming[index] > 0 {
            continue;
        }
        propagate_monotone_selected_display_flow(
            index,
            selected_flow,
            selected,
            downstream,
            node_kinds,
            &mut visited,
        );
    }

    for index in 0..selected.len() {
        if selected[index] && !visited[index] {
            propagate_monotone_selected_display_flow(
                index,
                selected_flow,
                selected,
                downstream,
                node_kinds,
                &mut visited,
            );
        }
    }
}

pub(super) fn propagate_monotone_selected_display_flow(
    start: usize,
    selected_flow: &mut [f32],
    selected: &[bool],
    downstream: &[Option<usize>],
    node_kinds: &[GraphDrainageNodeKind],
    visited: &mut [bool],
) {
    let mut current = start;
    let mut carried = 0.0_f32;
    let mut guard = 0_usize;

    while selected.get(current).copied().unwrap_or(false) && !visited[current] {
        visited[current] = true;
        selected_flow[current] = selected_flow[current].max(carried);
        carried = carried.max(selected_flow[current]);

        let Some(target) = downstream[current] else {
            break;
        };
        if node_kinds
            .get(target)
            .copied()
            .is_some_and(|kind| kind == GraphDrainageNodeKind::LakeInlet)
        {
            break;
        }
        if !selected.get(target).copied().unwrap_or(false) {
            break;
        }

        current = target;
        guard += 1;
        if guard > selected.len() {
            break;
        }
    }
}

pub(super) fn build_nodes(
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

pub(super) fn build_segments(
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
            let length = squared_distance(
                patch.corners[index].position,
                patch.corners[target].position,
            )
            .sqrt()
            .max(f32::EPSILON);
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
                local_slope: (drop / length).max(0.0),
            })
        })
        .collect::<Vec<_>>();
    segments.sort_by_key(|segment| segment.id.0);
    segments
}

pub(super) fn river_role(flow: f32) -> GraphHydrologyRole {
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

pub(super) fn node_id(corner: VoronoiCornerId) -> GraphDrainageNodeId {
    GraphDrainageNodeId(splitmix64(corner.0 ^ 0x75a7_2f38_1e91_4d0c))
}

pub(super) fn squared_distance(a: WorldPlanePoint, b: WorldPlanePoint) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    dx * dx + dz * dz
}
