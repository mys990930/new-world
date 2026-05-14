mod discharge;
mod routing;
mod selection;
mod topology;
mod types;

use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

use super::biome::{GraphBiomeWaterRole, classify_graph_biome};
use super::graph::VoronoiGraphPatch;
use super::macro_map::{GraphMacroMap, MacroSurfaceKind};
use discharge::{build_nodes, build_segments, resolve_selected_flow_accumulation};
use routing::{
    corner_adjacency, is_terminal_outlet, resolve_downstream, resolve_first_downstream_lakes,
    resolve_flow_accumulation, resolve_terminal_indices, resolve_watersheds,
};
use selection::{resolve_lake_inlet_policies, resolve_lake_terminal_policies, select_river_paths};
use topology::{resolve_lake_contact_topology, resolve_node_kinds, resolve_topology_stats};
use types::validate_hydrology_config;

#[cfg(test)]
use super::graph::VoronoiCornerId;

pub use types::{
    DEFAULT_HEADWATER_ELEVATION, DEFAULT_HEADWATER_SOURCE_HYDRATION_FLOOR,
    DEFAULT_LAKE_AREA_UNITS_PER_CHAIN, DEFAULT_LAKE_DISCHARGE_CAP_CEILING,
    DEFAULT_LAKE_DISCHARGE_CAP_FLOOR, DEFAULT_LAKE_DISCHARGE_CAP_PER_AREA,
    DEFAULT_LAKE_DISCHARGE_RANGE_PER_AREA, DEFAULT_LAKE_INLET_OUTLET_MIN_EDGE_HOPS,
    DEFAULT_LAKE_MAX_INCOMING_CHAINS, DEFAULT_LAKE_MAX_OUTLETS_PER_COMPONENT,
    DEFAULT_LAKE_RIVER_FLOW_THRESHOLD_MULTIPLIER, DEFAULT_RIVER_FLOW_THRESHOLD,
    DEFAULT_TRIBUTARY_MAX_PATH_EDGES, DEFAULT_TRIBUTARY_PARALLEL_PATH_COMPARE_EDGES,
    DEFAULT_TRIBUTARY_PARALLEL_PATH_MIN_SPACING_BLOCKS, DEFAULT_TRIBUTARY_SOURCE_HYDRATION,
    DEFAULT_TRIBUTARY_SOURCE_MIN_SPACING_BLOCKS, DEFAULT_TRIBUTARY_SOURCE_THRESHOLD,
    GraphDrainageNode, GraphDrainageNodeId, GraphDrainageNodeKind, GraphHydrologyCorner,
    GraphHydrologyGraph, GraphHydrologyRole, GraphHydrologyTopologyStats,
    GraphLocalMinimumResolution, GraphRiverSegment, GraphRiverSegmentId, HydrologyConfig,
    WatershedId,
};

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
    let source_hydration = patch
        .corners
        .iter()
        .map(|corner| corner.base_fields.hydration)
        .collect::<Vec<_>>();
    let corner_positions = patch
        .corners
        .iter()
        .map(|corner| corner.position)
        .collect::<Vec<_>>();
    let selected_rivers = select_river_paths(
        &downstream,
        &downstream_edges,
        &flow_accumulation,
        &elevations,
        &source_hydration,
        Some(&corner_positions),
        &terminals,
        &lake_candidates,
        &resolutions,
        &terminal_indices,
        &first_downstream_lakes,
        &lake_policies,
        &lake_inlet_policies,
        &lake_topology,
        &adjacency,
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
        &selected,
        &downstream,
        &lake_topology,
        &node_kinds,
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
        selected_rivers.disconnected_river_fragment_pruned_count,
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

pub fn apply_headwater_source_hydration_to_biomes(
    patch: &VoronoiGraphPatch,
    macro_map: &mut GraphMacroMap,
    hydrology: &GraphHydrologyGraph,
) -> usize {
    let headwater_sites = headwater_adjacent_sites(patch, hydrology);
    if headwater_sites.is_empty() {
        return 0;
    }

    let mut updated = 0;
    for cell in &mut macro_map.biomes {
        if !headwater_sites.contains(&cell.site) {
            continue;
        }
        if !matches!(
            cell.context.water_role,
            GraphBiomeWaterRole::Land | GraphBiomeWaterRole::DryBasin
        ) {
            continue;
        }

        let mut context = cell.context;
        let original_context = context;
        context.hydration = context
            .hydration
            .max(DEFAULT_HEADWATER_SOURCE_HYDRATION_FLOOR);
        if context.water_role == GraphBiomeWaterRole::DryBasin {
            context.water_role = GraphBiomeWaterRole::Land;
        }
        context = context.clamped();

        if context != original_context {
            cell.context = context;
            cell.biome = classify_graph_biome(context);
            updated += 1;
        }
    }

    updated
}

fn headwater_adjacent_sites(
    patch: &VoronoiGraphPatch,
    hydrology: &GraphHydrologyGraph,
) -> HashSet<super::graph::VoronoiSiteId> {
    let edge_sites = patch
        .edges
        .iter()
        .map(|edge| (edge.id, edge.sites))
        .collect::<HashMap<_, _>>();
    let mut sites = HashSet::new();

    for segment in &hydrology.segments {
        if segment.role != GraphHydrologyRole::Headwater {
            continue;
        }
        let Some(edge_sites) = edge_sites.get(&segment.edge) else {
            continue;
        };
        sites.insert(edge_sites[0]);
        sites.insert(edge_sites[1]);
    }

    sites
}

#[cfg(test)]
mod tests {
    use super::discharge::resolve_selected_flow_accumulation;
    use super::routing::{CornerNeighbor, resolve_terminal_indices};
    use super::selection::{
        lake_policy_for_area, resolve_lake_terminal_policies, select_river_paths,
    };
    use super::topology::{
        lake_components, lake_hop_distances, prune_disconnected_selected_fragments,
        remove_repeated_lake_contact_chains,
    };
    use super::types::LakeContactTopology;
    use super::*;
    use crate::world::generation::biome::GraphBiomeKind;
    use crate::world::generation::graph::{
        DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS, VoronoiEdgeId,
        VoronoiGraphConfig, VoronoiGraphPatchRequest, WorldPlanePoint,
        generate_voronoi_graph_patch,
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
                    local_slope: 0.01,
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
                    local_slope: 0.02,
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
        let (_, hydro) = (1..=32)
            .find_map(|seed| {
                let (patch, macro_map) = test_inputs(seed);
                let hydro = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());
                (!hydro.segments.is_empty()).then_some((patch, hydro))
            })
            .expect("default hydrology should select visible river chains in the seed scan");
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
        let nodes_by_id = hydro
            .nodes
            .iter()
            .map(|node| (node.id, node))
            .collect::<HashMap<_, _>>();

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

            if !segments_by_from.contains_key(&segment.to) {
                let node = nodes_by_id
                    .get(&segment.to)
                    .copied()
                    .expect("segment target node should exist");
                assert_eq!(
                    node.kind,
                    GraphDrainageNodeKind::Source,
                    "selected segment should continue to an outlet, explicit resolution, or a pruned tributary endpoint"
                );
            }
        }
    }

    #[test]
    fn selected_seed_scan_reaches_valid_terminal_with_monotone_display_flow() {
        let mut saw_selected_path = false;

        for seed in 1..=32 {
            let (patch, macro_map) = test_inputs(seed);
            let hydro = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());
            let nodes = nodes_by_id(&hydro);
            let outgoing = hydro
                .segments
                .iter()
                .map(|segment| (segment.from, segment))
                .collect::<HashMap<_, _>>();

            for start in &hydro.segments {
                saw_selected_path = true;
                let mut current = start;
                let mut previous_display = 0.0_f32;
                let mut guard = 0_usize;

                loop {
                    assert!(
                        current.flow_accumulation + 0.001 >= previous_display,
                        "seed {seed} selected display flow decreased: {:?}",
                        current
                    );
                    previous_display = previous_display.max(current.flow_accumulation);

                    let to = nodes
                        .get(&current.to)
                        .expect("selected segment target node should exist");
                    match to.kind {
                        GraphDrainageNodeKind::CoastOutlet | GraphDrainageNodeKind::LakeInlet => {
                            break;
                        }
                        GraphDrainageNodeKind::Lake | GraphDrainageNodeKind::Sink => {
                            panic!(
                                "seed {seed} ordinary selected path ended at non-ocean basin node: {:?}",
                                to
                            );
                        }
                        _ => {}
                    }

                    let Some(next) = outgoing.get(&current.to).copied() else {
                        panic!(
                            "seed {seed} selected ordinary path lost downstream terminal at node {:?}",
                            to
                        );
                    };
                    current = next;
                    guard += 1;
                    assert!(
                        guard <= hydro.segments.len(),
                        "seed {seed} selected path cycled without a valid terminal"
                    );
                }
            }
        }

        assert!(
            saw_selected_path,
            "bounded deterministic seed scan should include selected ordinary river paths"
        );
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
    fn selected_tributaries_prune_weaker_multi_incoming_branch() {
        let downstream = vec![Some(2), Some(2), Some(3), None];
        let downstream_edges = vec![
            Some(VoronoiEdgeId(40)),
            Some(VoronoiEdgeId(41)),
            Some(VoronoiEdgeId(42)),
            None,
        ];
        let flow = vec![14.0, 26.0, 48.0, 48.0];
        let elevations = vec![0.65, 0.70, 0.40, 0.0];
        let terminals = vec![false, false, false, true];
        let lake_candidates = vec![false; 4];
        let resolutions = vec![
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::OceanOutlet,
        ];
        let terminal_indices = resolve_terminal_indices(&downstream);
        let first_downstream_lakes = vec![None; 4];
        let lake_policies = vec![None; 4];
        let lake_inlet_policies = vec![None; 4];
        let lake_topology = LakeContactTopology {
            component_by_corner: vec![None; 4],
            contact_component_by_land_corner: vec![None; 4],
            inlet_vertices: vec![false; 4],
            outlet_vertices: vec![false; 4],
            inlet_land_vertices: vec![false; 4],
            outlet_land_vertices: vec![false; 4],
        };
        let source_hydration = vec![0.55, 0.55, 0.20, 0.0];
        let adjacency = vec![
            vec![CornerNeighbor {
                index: 2,
                edge: VoronoiEdgeId(40),
            }],
            vec![CornerNeighbor {
                index: 2,
                edge: VoronoiEdgeId(41),
            }],
            vec![
                CornerNeighbor {
                    index: 0,
                    edge: VoronoiEdgeId(40),
                },
                CornerNeighbor {
                    index: 1,
                    edge: VoronoiEdgeId(41),
                },
                CornerNeighbor {
                    index: 3,
                    edge: VoronoiEdgeId(42),
                },
            ],
            vec![CornerNeighbor {
                index: 2,
                edge: VoronoiEdgeId(42),
            }],
        ];
        let config = HydrologyConfig {
            river_flow_threshold: 10.0,
            ..HydrologyConfig::default()
        };

        let selected_rivers = select_river_paths(
            &downstream,
            &downstream_edges,
            &flow,
            &elevations,
            &source_hydration,
            None,
            &terminals,
            &lake_candidates,
            &resolutions,
            &terminal_indices,
            &first_downstream_lakes,
            &lake_policies,
            &lake_inlet_policies,
            &lake_topology,
            &adjacency,
            &HashMap::new(),
            config,
        );

        assert_eq!(selected_rivers.selected, vec![true, true, true, false]);
        assert_eq!(selected_rivers.duplicate_trunk_pruned_count, 0);

        let node_kinds = resolve_node_kinds(
            &selected_rivers.selected,
            &downstream,
            &flow,
            &terminals,
            &resolutions,
            &lake_topology,
            &lake_inlet_policies,
            config,
        );
        let stats = resolve_topology_stats(
            &selected_rivers.selected,
            &downstream,
            &lake_candidates,
            &resolutions,
            &downstream_edges,
            &HashMap::new(),
            &node_kinds,
            selected_rivers.duplicate_trunk_pruned_count,
            selected_rivers.repeated_lake_contact_pruned_count,
            selected_rivers.disconnected_river_fragment_pruned_count,
        );

        assert_eq!(node_kinds[2], GraphDrainageNodeKind::Confluence);
        assert_eq!(node_kinds[3], GraphDrainageNodeKind::CoastOutlet);
        assert_eq!(stats.invalid_river_intersection_count, 0);
        assert_eq!(stats.ambiguous_shared_corner_count, 0);
        assert_eq!(stats.invalid_lake_contact_count, 0);
    }

    #[test]
    fn multi_incoming_prune_caps_confluence_at_two_incoming() {
        let downstream = vec![Some(4), Some(4), Some(4), Some(4), Some(5), None];
        let downstream_edges = vec![
            Some(VoronoiEdgeId(50)),
            Some(VoronoiEdgeId(51)),
            Some(VoronoiEdgeId(52)),
            Some(VoronoiEdgeId(53)),
            Some(VoronoiEdgeId(54)),
            None,
        ];
        let flow = vec![20.0, 30.0, 40.0, 50.0, 140.0, 140.0];
        let elevations = vec![0.76, 0.74, 0.72, 0.70, 0.40, 0.0];
        let source_hydration = vec![0.55, 0.56, 0.57, 0.58, 0.0, 0.0];
        let terminals = vec![false, false, false, false, false, true];
        let lake_candidates = vec![false; 6];
        let resolutions = vec![
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::OceanOutlet,
        ];
        let lake_topology = LakeContactTopology {
            component_by_corner: vec![None; 6],
            contact_component_by_land_corner: vec![None; 6],
            inlet_vertices: vec![false; 6],
            outlet_vertices: vec![false; 6],
            inlet_land_vertices: vec![false; 6],
            outlet_land_vertices: vec![false; 6],
        };
        let adjacency = vec![
            vec![CornerNeighbor {
                index: 4,
                edge: VoronoiEdgeId(50),
            }],
            vec![CornerNeighbor {
                index: 4,
                edge: VoronoiEdgeId(51),
            }],
            vec![CornerNeighbor {
                index: 4,
                edge: VoronoiEdgeId(52),
            }],
            vec![CornerNeighbor {
                index: 4,
                edge: VoronoiEdgeId(53),
            }],
            vec![
                CornerNeighbor {
                    index: 0,
                    edge: VoronoiEdgeId(50),
                },
                CornerNeighbor {
                    index: 1,
                    edge: VoronoiEdgeId(51),
                },
                CornerNeighbor {
                    index: 2,
                    edge: VoronoiEdgeId(52),
                },
                CornerNeighbor {
                    index: 3,
                    edge: VoronoiEdgeId(53),
                },
                CornerNeighbor {
                    index: 5,
                    edge: VoronoiEdgeId(54),
                },
            ],
            vec![CornerNeighbor {
                index: 4,
                edge: VoronoiEdgeId(54),
            }],
        ];

        let selected_rivers = select_river_paths(
            &downstream,
            &downstream_edges,
            &flow,
            &elevations,
            &source_hydration,
            None,
            &terminals,
            &lake_candidates,
            &resolutions,
            &resolve_terminal_indices(&downstream),
            &[None; 6],
            &[None; 6],
            &[None; 6],
            &lake_topology,
            &adjacency,
            &HashMap::new(),
            HydrologyConfig {
                river_flow_threshold: 20.0,
                ..HydrologyConfig::default()
            },
        );

        assert_eq!(
            selected_rivers.selected,
            vec![false, false, true, true, true, false],
            "a selected confluence may keep only the two strongest incoming branches"
        );
        assert_eq!(selected_rivers.duplicate_trunk_pruned_count, 0);
        assert_eq!(selected_rivers.disconnected_river_fragment_pruned_count, 0);

        let mut incoming = vec![0_u32; downstream.len()];
        for (index, is_selected) in selected_rivers.selected.iter().copied().enumerate() {
            if is_selected {
                incoming[downstream[index].expect("selected edge should have target")] += 1;
            }
        }
        assert!(
            incoming.into_iter().all(|count| count <= 2),
            "selected geometry must still have at most two incoming segments per vertex"
        );
    }

    #[test]
    fn final_reachability_prunes_selected_high_flow_sink_fragment() {
        let downstream = vec![Some(1), None, Some(3), None];
        let mut selected = vec![true, false, true, false];
        let terminals = vec![false, false, false, true];
        let resolutions = vec![
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::Sink,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::OceanOutlet,
        ];
        let lake_topology = LakeContactTopology {
            component_by_corner: vec![None; 4],
            contact_component_by_land_corner: vec![None; 4],
            inlet_vertices: vec![false; 4],
            outlet_vertices: vec![false; 4],
            inlet_land_vertices: vec![false; 4],
            outlet_land_vertices: vec![false; 4],
        };

        let pruned = prune_disconnected_selected_fragments(
            &mut selected,
            &downstream,
            &terminals,
            &resolutions,
            &lake_topology,
        );

        assert_eq!(
            selected,
            vec![false, false, true, false],
            "ordinary selected river fragments must not survive just because they end in a local sink"
        );
        assert_eq!(pruned, 1);
    }

    #[test]
    fn selected_display_flow_is_propagated_downstream() {
        let raw_flow = vec![12.0, 80.0, 24.0, 24.0];
        let selected = vec![true, true, true, false];
        let downstream = vec![Some(1), Some(2), Some(3), None];
        let selected_flow = resolve_selected_flow_accumulation(
            &raw_flow,
            &selected,
            &downstream,
            &LakeContactTopology {
                component_by_corner: vec![None; 4],
                contact_component_by_land_corner: vec![None; 4],
                inlet_vertices: vec![false; 4],
                outlet_vertices: vec![false; 4],
                inlet_land_vertices: vec![false; 4],
                outlet_land_vertices: vec![false; 4],
            },
            &[
                GraphDrainageNodeKind::Source,
                GraphDrainageNodeKind::Source,
                GraphDrainageNodeKind::Source,
                GraphDrainageNodeKind::CoastOutlet,
            ],
        );

        assert_eq!(
            &selected_flow[..3],
            &[12.0, 80.0, 80.0],
            "display discharge on a selected ordinary path should not shrink downstream"
        );
        assert_eq!(
            raw_flow[2], 24.0,
            "raw flow remains the diagnostic/source ledger"
        );
    }

    #[test]
    fn selected_display_flow_propagates_through_lake_system_transition() {
        let raw_flow = vec![120.0, 0.0, 0.0, 12.0, 18.0, 18.0];
        let selected = vec![true, false, false, true, true, false];
        let downstream = vec![Some(1), Some(2), Some(3), Some(4), Some(5), None];
        let lake_topology = LakeContactTopology {
            component_by_corner: vec![None, None, Some(0), None, None, None],
            contact_component_by_land_corner: vec![None, Some(0), None, Some(0), None, None],
            inlet_vertices: vec![false, false, true, false, false, false],
            outlet_vertices: vec![false, false, true, false, false, false],
            inlet_land_vertices: vec![false, true, false, false, false, false],
            outlet_land_vertices: vec![false, false, false, true, false, false],
        };
        let node_kinds = vec![
            GraphDrainageNodeKind::Source,
            GraphDrainageNodeKind::LakeInlet,
            GraphDrainageNodeKind::Lake,
            GraphDrainageNodeKind::LakeOutlet,
            GraphDrainageNodeKind::Source,
            GraphDrainageNodeKind::CoastOutlet,
        ];

        let selected_flow = resolve_selected_flow_accumulation(
            &raw_flow,
            &selected,
            &downstream,
            &lake_topology,
            &node_kinds,
        );

        assert_eq!(
            selected_flow[0], 120.0,
            "the upstream lake inlet segment should keep canonical river-system Q"
        );
        assert_eq!(
            selected_flow[3], 120.0,
            "lake outlet should inherit upstream lake-inlet river-system Q instead of restarting small"
        );
        assert_eq!(
            selected_flow[4], 120.0,
            "ordinary downstream segments after the lake outlet should keep the inherited system Q"
        );
    }

    #[test]
    fn default_selection_threshold_prunes_small_tributary_starts() {
        let downstream = vec![Some(2), Some(2), Some(3), None];
        let downstream_edges = vec![
            Some(VoronoiEdgeId(40)),
            Some(VoronoiEdgeId(41)),
            Some(VoronoiEdgeId(42)),
            None,
        ];
        let flow = vec![2.0, 3.0, 4.0, 4.0];
        let elevations = vec![0.65, 0.70, 0.40, 0.0];
        let terminals = vec![false, false, false, true];
        let lake_candidates = vec![false; 4];
        let resolutions = vec![
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::OceanOutlet,
        ];
        let terminal_indices = resolve_terminal_indices(&downstream);
        let source_hydration = vec![0.55, 0.55, 0.20, 0.0];
        let adjacency = vec![
            vec![CornerNeighbor {
                index: 2,
                edge: VoronoiEdgeId(40),
            }],
            vec![CornerNeighbor {
                index: 2,
                edge: VoronoiEdgeId(41),
            }],
            vec![
                CornerNeighbor {
                    index: 0,
                    edge: VoronoiEdgeId(40),
                },
                CornerNeighbor {
                    index: 1,
                    edge: VoronoiEdgeId(41),
                },
                CornerNeighbor {
                    index: 3,
                    edge: VoronoiEdgeId(42),
                },
            ],
            vec![CornerNeighbor {
                index: 2,
                edge: VoronoiEdgeId(42),
            }],
        ];
        let selected_rivers = select_river_paths(
            &downstream,
            &downstream_edges,
            &flow,
            &elevations,
            &source_hydration,
            None,
            &terminals,
            &lake_candidates,
            &resolutions,
            &terminal_indices,
            &[None; 4],
            &[None; 4],
            &[None; 4],
            &LakeContactTopology {
                component_by_corner: vec![None; 4],
                contact_component_by_land_corner: vec![None; 4],
                inlet_vertices: vec![false; 4],
                outlet_vertices: vec![false; 4],
                inlet_land_vertices: vec![false; 4],
                outlet_land_vertices: vec![false; 4],
            },
            &adjacency,
            &HashMap::new(),
            HydrologyConfig::default(),
        );

        assert_eq!(
            DEFAULT_RIVER_FLOW_THRESHOLD, 30.0,
            "launch default keeps sub-threshold marginal tributaries out of the selected graph"
        );
        assert_eq!(
            selected_rivers.selected,
            vec![false; 4],
            "the main river threshold should not be bypassed to create sub-threshold tributary starts"
        );
    }

    #[test]
    fn stricter_default_threshold_prunes_marginal_tributary_starts() {
        let downstream = vec![Some(1), Some(2), Some(3), None];
        let downstream_edges = vec![
            Some(VoronoiEdgeId(60)),
            Some(VoronoiEdgeId(61)),
            Some(VoronoiEdgeId(62)),
            None,
        ];
        let flow = vec![8.0, 18.0, 21.0, 21.0];
        let elevations = vec![0.64, 0.44, 0.30, 0.0];
        let source_hydration = vec![0.56, 0.42, 0.25, 0.0];
        let terminals = vec![false, false, false, true];
        let lake_candidates = vec![false; 4];
        let resolutions = vec![
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::OceanOutlet,
        ];
        let selected = select_river_paths(
            &downstream,
            &downstream_edges,
            &flow,
            &elevations,
            &source_hydration,
            None,
            &terminals,
            &lake_candidates,
            &resolutions,
            &resolve_terminal_indices(&downstream),
            &[None; 4],
            &[None; 4],
            &[None; 4],
            &LakeContactTopology {
                component_by_corner: vec![None; 4],
                contact_component_by_land_corner: vec![None; 4],
                inlet_vertices: vec![false; 4],
                outlet_vertices: vec![false; 4],
                inlet_land_vertices: vec![false; 4],
                outlet_land_vertices: vec![false; 4],
            },
            &vec![Vec::new(); 4],
            &HashMap::new(),
            HydrologyConfig {
                river_flow_threshold: 50.0,
                ..HydrologyConfig::default()
            },
        )
        .selected;

        assert_eq!(
            selected,
            vec![false; 4],
            "a terrain-like source should still be pruned when downstream path potential stays below the bumped default threshold"
        );
    }

    #[test]
    fn source_candidate_selects_headwater_before_downstream_threshold_crossing() {
        let downstream = vec![Some(1), Some(2), Some(3), None];
        let downstream_edges = vec![
            Some(VoronoiEdgeId(50)),
            Some(VoronoiEdgeId(51)),
            Some(VoronoiEdgeId(52)),
            None,
        ];
        let flow = vec![3.0, 12.0, 56.0, 56.0];
        let elevations = vec![0.62, 0.28, 0.22, 0.0];
        let source_hydration = vec![0.55, 0.0, 0.0, 0.0];
        let terminals = vec![false, false, false, true];
        let lake_candidates = vec![false; 4];
        let resolutions = vec![
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::OceanOutlet,
        ];
        let selected = select_river_paths(
            &downstream,
            &downstream_edges,
            &flow,
            &elevations,
            &source_hydration,
            None,
            &terminals,
            &lake_candidates,
            &resolutions,
            &resolve_terminal_indices(&downstream),
            &[None; 4],
            &[None; 4],
            &[None; 4],
            &LakeContactTopology {
                component_by_corner: vec![None; 4],
                contact_component_by_land_corner: vec![None; 4],
                inlet_vertices: vec![false; 4],
                outlet_vertices: vec![false; 4],
                inlet_land_vertices: vec![false; 4],
                outlet_land_vertices: vec![false; 4],
            },
            &vec![Vec::new(); 4],
            &HashMap::new(),
            HydrologyConfig {
                river_flow_threshold: 50.0,
                ..HydrologyConfig::default()
            },
        )
        .selected;

        assert_eq!(
            selected,
            vec![true, true, true, false],
            "selection should start at the hydrated terrain source, not only at the first threshold crossing"
        );
    }

    #[test]
    fn explicit_tributary_threshold_increases_sources_without_extending_mainstem() {
        let downstream = vec![Some(3), Some(4), Some(4), Some(4), Some(5), None];
        let downstream_edges = vec![
            Some(VoronoiEdgeId(70)),
            Some(VoronoiEdgeId(71)),
            Some(VoronoiEdgeId(72)),
            Some(VoronoiEdgeId(73)),
            Some(VoronoiEdgeId(74)),
            None,
        ];
        let flow = vec![18.0, 16.0, 15.0, 140.0, 190.0, 190.0];
        let elevations = vec![0.74, 0.73, 0.72, 0.50, 0.30, 0.0];
        let source_hydration = vec![0.58, 0.57, 0.56, 1.0, 0.0, 0.0];
        let corner_positions = vec![
            WorldPlanePoint::new(-1600.0, -512.0),
            WorldPlanePoint::new(1600.0, 512.0),
            WorldPlanePoint::new(0.0, 1600.0),
            WorldPlanePoint::new(-512.0, 0.0),
            WorldPlanePoint::new(512.0, 0.0),
            WorldPlanePoint::new(1024.0, 0.0),
        ];
        let terminals = vec![false, false, false, false, false, true];
        let lake_candidates = vec![false; 6];
        let resolutions = vec![
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::OceanOutlet,
        ];
        let adjacency = vec![
            vec![CornerNeighbor {
                index: 3,
                edge: VoronoiEdgeId(70),
            }],
            vec![CornerNeighbor {
                index: 4,
                edge: VoronoiEdgeId(71),
            }],
            vec![CornerNeighbor {
                index: 4,
                edge: VoronoiEdgeId(72),
            }],
            vec![
                CornerNeighbor {
                    index: 0,
                    edge: VoronoiEdgeId(70),
                },
                CornerNeighbor {
                    index: 4,
                    edge: VoronoiEdgeId(73),
                },
            ],
            vec![
                CornerNeighbor {
                    index: 1,
                    edge: VoronoiEdgeId(71),
                },
                CornerNeighbor {
                    index: 2,
                    edge: VoronoiEdgeId(72),
                },
                CornerNeighbor {
                    index: 3,
                    edge: VoronoiEdgeId(73),
                },
                CornerNeighbor {
                    index: 5,
                    edge: VoronoiEdgeId(74),
                },
            ],
            vec![CornerNeighbor {
                index: 4,
                edge: VoronoiEdgeId(74),
            }],
        ];
        let topology = LakeContactTopology {
            component_by_corner: vec![None; 6],
            contact_component_by_land_corner: vec![None; 6],
            inlet_vertices: vec![false; 6],
            outlet_vertices: vec![false; 6],
            inlet_land_vertices: vec![false; 6],
            outlet_land_vertices: vec![false; 6],
        };
        let strict = select_river_paths(
            &downstream,
            &downstream_edges,
            &flow,
            &elevations,
            &source_hydration,
            Some(&corner_positions),
            &terminals,
            &lake_candidates,
            &resolutions,
            &resolve_terminal_indices(&downstream),
            &[None; 6],
            &[None; 6],
            &[None; 6],
            &topology,
            &adjacency,
            &HashMap::new(),
            HydrologyConfig {
                river_flow_threshold: 100.0,
                tributary_source_threshold: 0.95,
                ..HydrologyConfig::default()
            },
        )
        .selected;
        let dense = select_river_paths(
            &downstream,
            &downstream_edges,
            &flow,
            &elevations,
            &source_hydration,
            Some(&corner_positions),
            &terminals,
            &lake_candidates,
            &resolutions,
            &resolve_terminal_indices(&downstream),
            &[None; 6],
            &[None; 6],
            &[None; 6],
            &topology,
            &adjacency,
            &HashMap::new(),
            HydrologyConfig {
                river_flow_threshold: 100.0,
                tributary_source_threshold: 0.50,
                ..HydrologyConfig::default()
            },
        )
        .selected;

        assert_eq!(
            strict,
            vec![false, false, false, true, true, false],
            "strict tributary source threshold should keep only the mainstem"
        );
        assert_eq!(
            dense,
            vec![true, true, false, true, true, false],
            "lower tributary source threshold should admit side sources that merge into the selected mainstem"
        );
        assert_eq!(
            strict[3..],
            dense[3..],
            "tributary density must not work by lengthening the mainstem path"
        );
    }

    #[test]
    fn clustered_high_hydration_tributaries_keep_best_spaced_source() {
        let downstream = vec![Some(3), Some(4), Some(3), Some(4), Some(5), None];
        let downstream_edges = vec![
            Some(VoronoiEdgeId(90)),
            Some(VoronoiEdgeId(91)),
            Some(VoronoiEdgeId(92)),
            Some(VoronoiEdgeId(93)),
            Some(VoronoiEdgeId(94)),
            None,
        ];
        let flow = vec![18.0, 17.0, 140.0, 180.0, 220.0, 220.0];
        let elevations = vec![0.74, 0.73, 0.70, 0.50, 0.30, 0.0];
        let source_hydration = vec![0.58, 0.57, 1.0, 0.0, 0.0, 0.0];
        let corner_positions = vec![
            WorldPlanePoint::new(0.0, 0.0),
            WorldPlanePoint::new(96.0, 0.0),
            WorldPlanePoint::new(-640.0, 0.0),
            WorldPlanePoint::new(512.0, 0.0),
            WorldPlanePoint::new(512.0, 320.0),
            WorldPlanePoint::new(512.0, 640.0),
        ];
        let terminals = vec![false, false, false, false, false, true];
        let lake_candidates = vec![false; 6];
        let resolutions = vec![
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::OceanOutlet,
        ];
        let adjacency = vec![
            vec![CornerNeighbor {
                index: 3,
                edge: VoronoiEdgeId(90),
            }],
            vec![CornerNeighbor {
                index: 4,
                edge: VoronoiEdgeId(91),
            }],
            vec![CornerNeighbor {
                index: 3,
                edge: VoronoiEdgeId(92),
            }],
            vec![
                CornerNeighbor {
                    index: 0,
                    edge: VoronoiEdgeId(90),
                },
                CornerNeighbor {
                    index: 2,
                    edge: VoronoiEdgeId(92),
                },
                CornerNeighbor {
                    index: 4,
                    edge: VoronoiEdgeId(93),
                },
            ],
            vec![
                CornerNeighbor {
                    index: 1,
                    edge: VoronoiEdgeId(91),
                },
                CornerNeighbor {
                    index: 3,
                    edge: VoronoiEdgeId(93),
                },
                CornerNeighbor {
                    index: 5,
                    edge: VoronoiEdgeId(94),
                },
            ],
            vec![CornerNeighbor {
                index: 4,
                edge: VoronoiEdgeId(94),
            }],
        ];

        let selected = select_river_paths(
            &downstream,
            &downstream_edges,
            &flow,
            &elevations,
            &source_hydration,
            Some(&corner_positions),
            &terminals,
            &lake_candidates,
            &resolutions,
            &resolve_terminal_indices(&downstream),
            &[None; 6],
            &[None; 6],
            &[None; 6],
            &LakeContactTopology {
                component_by_corner: vec![None; 6],
                contact_component_by_land_corner: vec![None; 6],
                inlet_vertices: vec![false; 6],
                outlet_vertices: vec![false; 6],
                inlet_land_vertices: vec![false; 6],
                outlet_land_vertices: vec![false; 6],
            },
            &adjacency,
            &HashMap::new(),
            HydrologyConfig {
                river_flow_threshold: 100.0,
                tributary_source_threshold: 0.50,
                tributary_source_min_spacing_blocks: 256.0,
                tributary_parallel_path_min_spacing_blocks: 192.0,
                ..HydrologyConfig::default()
            },
        )
        .selected;

        assert_eq!(
            selected,
            vec![true, false, true, true, true, false],
            "clustered hydrated tributary candidates should keep the best source and suppress adjacent parallel starts"
        );
    }

    #[test]
    fn explicit_tributary_merge_preserves_downstream_system_q() {
        let raw_flow = vec![28.0, 92.0, 130.0, 130.0];
        let selected = vec![true, true, true, false];
        let downstream = vec![Some(1), Some(2), Some(3), None];
        let selected_flow = resolve_selected_flow_accumulation(
            &raw_flow,
            &selected,
            &downstream,
            &LakeContactTopology {
                component_by_corner: vec![None; 4],
                contact_component_by_land_corner: vec![None; 4],
                inlet_vertices: vec![false; 4],
                outlet_vertices: vec![false; 4],
                inlet_land_vertices: vec![false; 4],
                outlet_land_vertices: vec![false; 4],
            },
            &[
                GraphDrainageNodeKind::Source,
                GraphDrainageNodeKind::Confluence,
                GraphDrainageNodeKind::Source,
                GraphDrainageNodeKind::CoastOutlet,
            ],
        );

        assert_eq!(
            selected_flow[1], 92.0,
            "mainstem segment should keep raw accumulated Q at the tributary merge"
        );
        assert!(
            selected_flow[2] >= raw_flow[0] + raw_flow[1],
            "downstream selected river-system Q should include tributary contribution"
        );
    }

    #[test]
    fn explicit_tributaries_do_not_intersect_before_mainstem_merge() {
        let downstream = vec![Some(2), Some(2), Some(3), Some(4), None];
        let downstream_edges = vec![
            Some(VoronoiEdgeId(80)),
            Some(VoronoiEdgeId(81)),
            Some(VoronoiEdgeId(82)),
            Some(VoronoiEdgeId(83)),
            None,
        ];
        let flow = vec![18.0, 16.0, 34.0, 140.0, 140.0];
        let elevations = vec![0.74, 0.73, 0.56, 0.50, 0.0];
        let source_hydration = vec![0.58, 0.57, 0.20, 1.0, 0.0];
        let terminals = vec![false, false, false, false, true];
        let lake_candidates = vec![false; 5];
        let resolutions = vec![
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::OceanOutlet,
        ];
        let adjacency = vec![
            vec![CornerNeighbor {
                index: 2,
                edge: VoronoiEdgeId(80),
            }],
            vec![CornerNeighbor {
                index: 2,
                edge: VoronoiEdgeId(81),
            }],
            vec![
                CornerNeighbor {
                    index: 0,
                    edge: VoronoiEdgeId(80),
                },
                CornerNeighbor {
                    index: 1,
                    edge: VoronoiEdgeId(81),
                },
                CornerNeighbor {
                    index: 3,
                    edge: VoronoiEdgeId(82),
                },
            ],
            vec![
                CornerNeighbor {
                    index: 2,
                    edge: VoronoiEdgeId(82),
                },
                CornerNeighbor {
                    index: 4,
                    edge: VoronoiEdgeId(83),
                },
            ],
            vec![CornerNeighbor {
                index: 3,
                edge: VoronoiEdgeId(83),
            }],
        ];

        let selected = select_river_paths(
            &downstream,
            &downstream_edges,
            &flow,
            &elevations,
            &source_hydration,
            None,
            &terminals,
            &lake_candidates,
            &resolutions,
            &resolve_terminal_indices(&downstream),
            &[None; 5],
            &[None; 5],
            &[None; 5],
            &LakeContactTopology {
                component_by_corner: vec![None; 5],
                contact_component_by_land_corner: vec![None; 5],
                inlet_vertices: vec![false; 5],
                outlet_vertices: vec![false; 5],
                inlet_land_vertices: vec![false; 5],
                outlet_land_vertices: vec![false; 5],
            },
            &adjacency,
            &HashMap::new(),
            HydrologyConfig {
                river_flow_threshold: 100.0,
                tributary_source_threshold: 0.50,
                ..HydrologyConfig::default()
            },
        )
        .selected;

        assert_eq!(
            selected,
            vec![true, false, true, true, false],
            "the stronger tributary may keep the shared path, while the weaker intersecting source is skipped"
        );
    }

    #[test]
    fn threshold_crossing_without_source_context_is_not_selected() {
        let downstream = vec![Some(1), Some(2), Some(3), None];
        let downstream_edges = vec![
            Some(VoronoiEdgeId(50)),
            Some(VoronoiEdgeId(51)),
            Some(VoronoiEdgeId(52)),
            None,
        ];
        let flow = vec![4.0, 44.0, 56.0, 56.0];
        let elevations = vec![0.16, 0.14, 0.12, 0.0];
        let source_hydration = vec![0.10, 0.10, 0.10, 0.0];
        let terminals = vec![false, false, false, true];
        let lake_candidates = vec![false; 4];
        let resolutions = vec![
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::None,
            GraphLocalMinimumResolution::OceanOutlet,
        ];
        let selected = select_river_paths(
            &downstream,
            &downstream_edges,
            &flow,
            &elevations,
            &source_hydration,
            None,
            &terminals,
            &lake_candidates,
            &resolutions,
            &resolve_terminal_indices(&downstream),
            &[None; 4],
            &[None; 4],
            &[None; 4],
            &LakeContactTopology {
                component_by_corner: vec![None; 4],
                contact_component_by_land_corner: vec![None; 4],
                inlet_vertices: vec![false; 4],
                outlet_vertices: vec![false; 4],
                inlet_land_vertices: vec![false; 4],
                outlet_land_vertices: vec![false; 4],
            },
            &vec![Vec::new(); 4],
            &HashMap::new(),
            HydrologyConfig::default(),
        )
        .selected;

        assert_eq!(
            selected,
            vec![false; 4],
            "a downstream threshold crossing should not create a selected river without a source-like start"
        );
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
        let lake_system_segments = hydro
            .segments
            .iter()
            .filter(|segment| segment.flow_accumulation + 0.001 >= segment.raw_flow_accumulation)
            .count();

        assert!(
            lake_resolutions >= 2,
            "seed 42 default macro preview window should resolve inland water as lakes, got {lake_resolutions}"
        );
        assert_eq!(lake_nodes, lake_resolutions);
        assert!(
            lake_system_segments > 0,
            "selected rivers should expose canonical river-system Q instead of lake-capped display Q"
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
        assert_eq!(
            invalid_shared_corner_count(&hydro),
            0,
            "selected river graph should not leave multiple selected incoming segments at one vertex"
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
            &[0.55, 0.55, 0.55, 0.0, 0.0, 0.0, 0.0],
            None,
            &terminals,
            &lake_candidates,
            &resolutions,
            &terminal_indices,
            &first_downstream_lakes,
            &lake_policies,
            &lake_inlet_policies,
            &lake_topology,
            &vec![Vec::new(); 7],
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
            &[0.55, 0.0, 0.0, 0.0],
            None,
            &terminals,
            &lake_candidates,
            &resolutions,
            &terminal_indices,
            &first_downstream_lakes,
            &lake_policies,
            &lake_inlet_policies,
            &lake_topology,
            &vec![Vec::new(); 4],
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
    fn lake_terminal_policy_does_not_cap_canonical_system_q() {
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
            &[false; 4],
            &[None; 4],
            &lake_topology,
            &[GraphDrainageNodeKind::Source; 4],
        );
        let policy = lake_policies[3].expect("lake terminal should have policy");

        assert!(policy.discharge_cap < raw_flow[2]);
        assert!(
            policy.discharge_cap <= DEFAULT_LAKE_DISCHARGE_CAP_CEILING,
            "lake local shape policy should still expose a conservative cap"
        );
        assert_eq!(
            selected_flow[2], raw_flow[2],
            "canonical river-system Q should not be overwritten by the lake local shape cap"
        );
        assert_eq!(
            raw_flow[2], 220.0,
            "raw hydrology ledger should remain unchanged"
        );
    }

    #[test]
    fn lake_inlet_endpoint_segment_keeps_canonical_system_q() {
        let raw_flow = vec![180.0, 180.0, 0.0];
        let downstream = vec![Some(1), Some(2), None];
        let config = HydrologyConfig::default();
        let lake_policy = lake_policy_for_area(24, 2, config);
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
            &[true, false, false],
            &downstream,
            &lake_topology,
            &node_kinds,
        );

        assert!(lake_policy.discharge_cap < raw_flow[0]);
        assert_eq!(
            selected_flow[0], raw_flow[0],
            "the segment ending at a LakeInlet marker should keep canonical system Q"
        );
        assert_eq!(raw_flow[0], 180.0);
    }

    #[test]
    fn lake_terminal_canonical_q_can_match_ocean_terminal_flow() {
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
            &[false; 5],
            &[None; 5],
            &lake_topology,
            &[GraphDrainageNodeKind::Source; 5],
        );
        let lake_policy = lake_policies[3].expect("lake terminal should have local shape policy");

        assert_eq!(
            selected_flow[4], raw_flow[4],
            "ocean terminal flow should use canonical raw/system Q"
        );
        assert!(lake_policy.discharge_cap < raw_flow[2]);
        assert!(
            selected_flow[2] > lake_policy.discharge_cap,
            "lake local cap should not overwrite canonical river-system Q"
        );
    }

    #[test]
    fn headwater_source_proximity_raises_final_biome_hydration() {
        let mut saw_headwater = false;

        for seed in 1..=32 {
            let (patch, mut macro_map) = test_inputs(seed);
            let hydrology = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());
            let headwater_sites = headwater_adjacent_sites(&patch, &hydrology);
            if headwater_sites.is_empty() {
                continue;
            }

            saw_headwater = true;
            let updated =
                apply_headwater_source_hydration_to_biomes(&patch, &mut macro_map, &hydrology);

            assert!(
                updated > 0
                    || headwater_sites.iter().all(|site| {
                        macro_map.biome(*site).is_some_and(|cell| {
                            cell.context.hydration >= DEFAULT_HEADWATER_SOURCE_HYDRATION_FLOOR
                        })
                    }),
                "headwater source pass should either update cells or find them already hydrated"
            );

            for site in headwater_sites {
                let Some(cell) = macro_map.biome(site) else {
                    continue;
                };
                if matches!(
                    cell.context.water_role,
                    GraphBiomeWaterRole::ShallowOcean
                        | GraphBiomeWaterRole::DeepOcean
                        | GraphBiomeWaterRole::Lake
                        | GraphBiomeWaterRole::Coast
                        | GraphBiomeWaterRole::Wetland
                ) {
                    continue;
                }

                assert!(
                    cell.context.hydration >= DEFAULT_HEADWATER_SOURCE_HYDRATION_FLOOR,
                    "source-adjacent land cell should not remain dry: site={site:?} cell={cell:?}"
                );
                assert!(
                    !is_dry_headwater_biome(cell.biome),
                    "source-adjacent land cell should not classify as a dry biome: site={site:?} cell={cell:?}"
                );
            }
        }

        assert!(
            saw_headwater,
            "bounded deterministic seed scan should include selected headwater segments"
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

    fn is_dry_headwater_biome(biome: GraphBiomeKind) -> bool {
        matches!(
            biome,
            GraphBiomeKind::Steppe
                | GraphBiomeKind::SemiDesert
                | GraphBiomeKind::Desert
                | GraphBiomeKind::DryShrubland
                | GraphBiomeKind::TemperateGrassland
        )
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
            *incoming.entry(segment.to).or_default() += 1;
            *outgoing.entry(segment.from).or_default() += 1;
        }

        incoming
            .into_iter()
            .filter(|(node, count)| {
                let outgoing_count = outgoing.get(node).copied().unwrap_or(0);
                let allowed = if outgoing_count == 1 { 2 } else { 1 };
                if *count <= allowed {
                    return false;
                }
                let kind = nodes[node].kind;
                !matches!(
                    kind,
                    GraphDrainageNodeKind::LakeInlet
                        | GraphDrainageNodeKind::Sink
                        | GraphDrainageNodeKind::CoastOutlet
                )
            })
            .count()
    }
}
