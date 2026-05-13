use std::collections::{HashMap, HashSet};

use super::graph::{VoronoiEdgeId, VoronoiGraphPatch, WorldPlanePoint};
use super::hydrology::{
    GraphDrainageNodeId, GraphDrainageNodeKind, GraphHydrologyGraph, GraphHydrologyRole,
    GraphRiverSegment, GraphRiverSegmentId,
};
use super::macro_map::{GraphMacroMap, MacroLakeEdgeClass};

pub const DEFAULT_RIVER_PLAN_TRUNK_FLOW: f32 = 1024.0;
const WIDTH_COEFFICIENT_MIN: f32 = 0.7;
const WIDTH_COEFFICIENT_MAX: f32 = 1.3;
const DEPTH_COEFFICIENT_MIN: f32 = 0.55;
const DEPTH_COEFFICIENT_MAX: f32 = 0.85;
const HYDRAULIC_DEPTH_Q_EXPONENT: f32 = 0.55;
const MORPHOLOGY_Q_MAX_DOWNSTREAM_MULTIPLIER: f32 = 2.35;
const MORPHOLOGY_Q_MAX_DOWNSTREAM_ADDITION: f32 = 36.0;
const MORPHOLOGY_Q_ALLOWED_UPSTREAM_FRACTION: f32 = 0.55;
const BED_DEPTH_ALLOWED_SHALLOWING_BLOCKS: f32 = 0.5;
const BROAD_VALLEY_DEPTH_ALLOWED_SHALLOWING_BLOCKS: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiverPlanConfig {
    pub trunk_flow_accumulation: f32,
}

impl Default for RiverPlanConfig {
    fn default() -> Self {
        Self {
            trunk_flow_accumulation: DEFAULT_RIVER_PLAN_TRUNK_FLOW,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RiverChainId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RiverReachId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiverReachType {
    Headwater,
    Upper,
    Middle,
    Lower,
    Trunk,
    LakeInlet,
    LakeOutlet,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RiverChain {
    pub id: RiverChainId,
    pub segment_ids: Vec<GraphRiverSegmentId>,
    pub terminal_kind: Option<GraphDrainageNodeKind>,
    pub length_blocks: f32,
    pub min_downstream_progress: f32,
    pub max_downstream_progress: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RiverReach {
    pub id: RiverReachId,
    pub chain_id: RiverChainId,
    pub segment_ids: Vec<GraphRiverSegmentId>,
    pub reach_type: RiverReachType,
    pub downstream_start: f32,
    pub downstream_end: f32,
    pub display_flow: f32,
    pub raw_flow: f32,
    pub upstream_area: f32,
    pub tributary_flow: f32,
    pub discharge_q: f32,
    pub morphology_discharge_q: f32,
    pub hydraulic_width_coefficient: f32,
    pub hydraulic_depth_coefficient: f32,
    pub velocity: f32,
    pub stream_order_hint: f32,
    pub broad_valley_width_blocks: f32,
    pub broad_valley_depth_blocks: f32,
    pub bed_width_blocks: f32,
    pub bed_depth_blocks: f32,
    pub bank_transition_width_blocks: f32,
    pub floodplain_width_blocks: f32,
    pub roughness_hint: f32,
    pub gravel_hint: f32,
    pub cutbank_hint: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiverSegmentPlan {
    pub segment_id: GraphRiverSegmentId,
    pub edge: VoronoiEdgeId,
    pub chain_id: RiverChainId,
    pub reach_id: RiverReachId,
    pub reach_type: RiverReachType,
    pub raw_flow: f32,
    pub display_flow: f32,
    pub upstream_area: f32,
    pub tributary_flow: f32,
    pub discharge_q: f32,
    pub hydraulic_width_coefficient: f32,
    pub hydraulic_depth_coefficient: f32,
    pub velocity: f32,
    pub downstream_progress: f32,
    pub chain_downstream_progress: f32,
    pub segment_length_blocks: f32,
    pub local_slope: f32,
    pub broad_valley_width_blocks: f32,
    pub broad_valley_depth_blocks: f32,
    pub bed_width_blocks: f32,
    pub bed_depth_blocks: f32,
    pub bank_transition_width_blocks: f32,
    pub floodplain_width_blocks: f32,
    pub roughness_hint: f32,
    pub gravel_hint: f32,
    pub cutbank_hint: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiverSegmentEndpointPlan {
    pub segment_id: GraphRiverSegmentId,
    pub edge: VoronoiEdgeId,
    pub from_position: WorldPlanePoint,
    pub to_position: WorldPlanePoint,
    pub downstream_position: WorldPlanePoint,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RiverPlanStats {
    pub selected_segment_count: usize,
    pub planned_segment_count: usize,
    pub chain_count: usize,
    pub reach_count: usize,
    pub invalid_lake_edge_segment_count: usize,
    pub missing_patch_edge_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RiverPlan {
    pub chains: Vec<RiverChain>,
    pub reaches: Vec<RiverReach>,
    pub segments: Vec<RiverSegmentPlan>,
    pub segment_endpoints: Vec<RiverSegmentEndpointPlan>,
    pub stats: RiverPlanStats,
}

impl RiverPlan {
    pub fn segment(&self, id: GraphRiverSegmentId) -> Option<&RiverSegmentPlan> {
        self.segments
            .iter()
            .find(|segment| segment.segment_id == id)
    }

    pub fn segments_for_edge(
        &self,
        edge: VoronoiEdgeId,
    ) -> impl Iterator<Item = &RiverSegmentPlan> {
        self.segments
            .iter()
            .filter(move |segment| segment.edge == edge)
    }

    pub fn endpoints(&self, id: GraphRiverSegmentId) -> Option<&RiverSegmentEndpointPlan> {
        self.segment_endpoints
            .iter()
            .find(|endpoints| endpoints.segment_id == id)
    }
}

pub fn build_river_plan(
    patch: &VoronoiGraphPatch,
    macro_map: &GraphMacroMap,
    hydrology: &GraphHydrologyGraph,
    config: RiverPlanConfig,
) -> RiverPlan {
    validate_config(config);

    if hydrology.segments.is_empty() {
        return RiverPlan::default();
    }

    let nodes = hydrology
        .nodes
        .iter()
        .map(|node| (node.id, node.kind))
        .collect::<HashMap<_, _>>();
    let edge_lengths = edge_lengths(patch);
    let macro_lake_classes = macro_map
        .edges
        .iter()
        .map(|edge| (edge.id, edge.lake_class))
        .collect::<HashMap<_, _>>();
    let chains = build_chains(hydrology, &nodes, &edge_lengths);
    let hydraulics = build_hydraulic_ledgers(hydrology, &chains);
    let node_positions = hydrology
        .nodes
        .iter()
        .map(|node| (node.id, node.position))
        .collect::<HashMap<_, _>>();
    let mut planned_segments = Vec::with_capacity(hydrology.segments.len());
    let mut segment_endpoints = Vec::with_capacity(hydrology.segments.len());
    let mut reaches = Vec::new();

    for chain in &chains {
        let chain_segments = chain
            .segment_ids
            .iter()
            .filter_map(|id| hydrology.segments.iter().find(|segment| segment.id == *id))
            .collect::<Vec<_>>();
        let reach_types = chain_segments
            .iter()
            .enumerate()
            .map(|(index, segment)| {
                classify_reach(
                    segment,
                    &nodes,
                    chain_progress(index, chain_segments.len()),
                    config,
                )
            })
            .collect::<Vec<_>>();
        let morphology_q = smoothed_morphology_discharge_q(
            &chain_segments,
            &reach_types,
            &hydraulics,
            chain.id,
            config,
        );
        let mut current_reach: Option<ReachBuild> = None;

        for (index, segment) in chain_segments.iter().enumerate() {
            let chain_progress = chain_progress(index, chain_segments.len());
            let reach_type = reach_types[index];
            let hydraulic = hydraulics
                .get(&segment.id)
                .copied()
                .unwrap_or_else(|| SegmentHydraulics::fallback(segment, chain.id));
            let morphology_discharge_q = morphology_q
                .get(&segment.id)
                .copied()
                .unwrap_or_else(|| base_morphology_discharge_q(segment, &hydraulic, reach_type));
            let morphology = segment_morphology(
                segment,
                &hydraulic,
                morphology_discharge_q,
                reach_type,
                chain_progress,
                config,
            );
            let reach_id = current_reach
                .as_ref()
                .filter(|reach| reach.reach_type == reach_type)
                .map(|reach| reach.id)
                .unwrap_or_else(|| {
                    if let Some(reach) = current_reach.take() {
                        reaches.push(reach.finish());
                    }
                    let id = RiverReachId(reaches.len() as u64);
                    current_reach = Some(ReachBuild::new(id, chain.id, reach_type));
                    id
                });

            let length = edge_lengths.get(&segment.edge).copied().unwrap_or(0.0);
            if let Some(reach) = &mut current_reach {
                reach.push(segment, &hydraulic, morphology_discharge_q, &morphology);
            }
            planned_segments.push(RiverSegmentPlan {
                segment_id: segment.id,
                edge: segment.edge,
                chain_id: chain.id,
                reach_id,
                reach_type,
                raw_flow: finite_nonnegative(segment.raw_flow_accumulation),
                display_flow: finite_nonnegative(segment.flow_accumulation),
                upstream_area: hydraulic.upstream_area,
                tributary_flow: hydraulic.tributary_flow,
                discharge_q: hydraulic.discharge_q,
                hydraulic_width_coefficient: hydraulic.width_coefficient,
                hydraulic_depth_coefficient: hydraulic.depth_coefficient,
                velocity: hydraulic.velocity,
                downstream_progress: finite_nonnegative(segment.downstream_progress),
                chain_downstream_progress: chain_progress,
                segment_length_blocks: length,
                local_slope: finite_nonnegative(segment.local_slope),
                broad_valley_width_blocks: morphology.broad_valley_width_blocks,
                broad_valley_depth_blocks: morphology.broad_valley_depth_blocks,
                bed_width_blocks: morphology.bed_width_blocks,
                bed_depth_blocks: morphology.bed_depth_blocks,
                bank_transition_width_blocks: morphology.bank_transition_width_blocks,
                floodplain_width_blocks: morphology.floodplain_width_blocks,
                roughness_hint: morphology.roughness_hint,
                gravel_hint: morphology.gravel_hint,
                cutbank_hint: morphology.cutbank_hint,
            });
            if let (Some(from_position), Some(to_position)) = (
                node_positions.get(&segment.from).copied(),
                node_positions.get(&segment.to).copied(),
            ) {
                segment_endpoints.push(RiverSegmentEndpointPlan {
                    segment_id: segment.id,
                    edge: segment.edge,
                    from_position,
                    to_position,
                    downstream_position: to_position,
                });
            }
        }

        if let Some(reach) = current_reach.take() {
            reaches.push(reach.finish());
        }
    }

    apply_downstream_depth_continuity(&chains, &mut planned_segments);
    refresh_reach_depths(&mut reaches, &planned_segments);

    planned_segments.sort_by_key(|segment| segment.segment_id.0);
    segment_endpoints.sort_by_key(|endpoints| endpoints.segment_id.0);
    reaches.sort_by_key(|reach| reach.id.0);
    let stats = RiverPlanStats {
        selected_segment_count: hydrology.segments.len(),
        planned_segment_count: planned_segments.len(),
        chain_count: chains.len(),
        reach_count: reaches.len(),
        invalid_lake_edge_segment_count: hydrology
            .segments
            .iter()
            .filter(|segment| {
                macro_lake_classes
                    .get(&segment.edge)
                    .copied()
                    .unwrap_or(MacroLakeEdgeClass::NonLake)
                    .excludes_selected_river()
            })
            .count(),
        missing_patch_edge_count: hydrology
            .segments
            .iter()
            .filter(|segment| !edge_lengths.contains_key(&segment.edge))
            .count(),
    };

    RiverPlan {
        chains,
        reaches,
        segments: planned_segments,
        segment_endpoints,
        stats,
    }
}

fn validate_config(config: RiverPlanConfig) {
    assert!(
        config.trunk_flow_accumulation.is_finite() && config.trunk_flow_accumulation > 0.0,
        "trunk_flow_accumulation must be positive and finite"
    );
}

fn build_chains(
    hydrology: &GraphHydrologyGraph,
    nodes: &HashMap<GraphDrainageNodeId, GraphDrainageNodeKind>,
    edge_lengths: &HashMap<VoronoiEdgeId, f32>,
) -> Vec<RiverChain> {
    let mut incoming = HashMap::<GraphDrainageNodeId, Vec<GraphRiverSegmentId>>::new();
    let mut outgoing = HashMap::<GraphDrainageNodeId, Vec<GraphRiverSegmentId>>::new();
    let segments = hydrology
        .segments
        .iter()
        .map(|segment| (segment.id, segment))
        .collect::<HashMap<_, _>>();

    for segment in &hydrology.segments {
        incoming.entry(segment.to).or_default().push(segment.id);
        outgoing.entry(segment.from).or_default().push(segment.id);
    }
    for ids in incoming.values_mut().chain(outgoing.values_mut()) {
        ids.sort_by_key(|id| id.0);
    }

    let mut starts = hydrology
        .segments
        .iter()
        .filter(|segment| {
            incoming.get(&segment.from).is_none_or(Vec::is_empty)
                || nodes
                    .get(&segment.from)
                    .is_some_and(|kind| matches!(kind, GraphDrainageNodeKind::LakeOutlet))
        })
        .map(|segment| segment.id)
        .collect::<Vec<_>>();
    starts.sort_by_key(|id| id.0);
    starts.dedup();

    let mut visited = HashSet::new();
    let mut chains = Vec::new();
    for start in starts {
        if visited.contains(&start) {
            continue;
        }
        chains.push(trace_chain(
            RiverChainId(chains.len() as u64),
            start,
            &segments,
            &outgoing,
            nodes,
            edge_lengths,
            &mut visited,
        ));
    }

    let mut leftovers = hydrology
        .segments
        .iter()
        .map(|segment| segment.id)
        .filter(|id| !visited.contains(id))
        .collect::<Vec<_>>();
    leftovers.sort_by_key(|id| id.0);
    for start in leftovers {
        if visited.contains(&start) {
            continue;
        }
        chains.push(trace_chain(
            RiverChainId(chains.len() as u64),
            start,
            &segments,
            &outgoing,
            nodes,
            edge_lengths,
            &mut visited,
        ));
    }

    chains
}

fn trace_chain(
    id: RiverChainId,
    start: GraphRiverSegmentId,
    segments: &HashMap<GraphRiverSegmentId, &GraphRiverSegment>,
    outgoing: &HashMap<GraphDrainageNodeId, Vec<GraphRiverSegmentId>>,
    nodes: &HashMap<GraphDrainageNodeId, GraphDrainageNodeKind>,
    edge_lengths: &HashMap<VoronoiEdgeId, f32>,
    visited: &mut HashSet<GraphRiverSegmentId>,
) -> RiverChain {
    let mut segment_ids = Vec::new();
    let mut current = Some(start);
    let mut terminal_kind = None;
    let mut length_blocks = 0.0;
    let mut min_downstream_progress = f32::INFINITY;
    let mut max_downstream_progress = f32::NEG_INFINITY;

    while let Some(id) = current {
        if !visited.insert(id) {
            break;
        }
        let Some(segment) = segments.get(&id).copied() else {
            break;
        };
        segment_ids.push(id);
        length_blocks += edge_lengths.get(&segment.edge).copied().unwrap_or(0.0);
        let progress = finite_nonnegative(segment.downstream_progress);
        min_downstream_progress = min_downstream_progress.min(progress);
        max_downstream_progress = max_downstream_progress.max(progress);

        let to_kind = nodes.get(&segment.to).copied();
        terminal_kind = to_kind;
        if to_kind.is_some_and(is_terminal_kind) {
            break;
        }

        current = outgoing
            .get(&segment.to)
            .and_then(|ids| ids.iter().copied().find(|next| !visited.contains(next)));
    }

    if !min_downstream_progress.is_finite() {
        min_downstream_progress = 0.0;
        max_downstream_progress = 0.0;
    }

    RiverChain {
        id,
        segment_ids,
        terminal_kind,
        length_blocks,
        min_downstream_progress,
        max_downstream_progress,
    }
}

fn is_terminal_kind(kind: GraphDrainageNodeKind) -> bool {
    matches!(
        kind,
        GraphDrainageNodeKind::LakeInlet
            | GraphDrainageNodeKind::Lake
            | GraphDrainageNodeKind::Sink
            | GraphDrainageNodeKind::CoastOutlet
    )
}

fn build_hydraulic_ledgers(
    hydrology: &GraphHydrologyGraph,
    chains: &[RiverChain],
) -> HashMap<GraphRiverSegmentId, SegmentHydraulics> {
    let mut incoming = HashMap::<GraphDrainageNodeId, Vec<GraphRiverSegmentId>>::new();
    let segments = hydrology
        .segments
        .iter()
        .map(|segment| (segment.id, segment))
        .collect::<HashMap<_, _>>();
    let segment_chains = chains
        .iter()
        .flat_map(|chain| {
            chain
                .segment_ids
                .iter()
                .copied()
                .map(move |segment_id| (segment_id, chain.id))
        })
        .collect::<HashMap<_, _>>();

    for segment in &hydrology.segments {
        incoming.entry(segment.to).or_default().push(segment.id);
    }
    for ids in incoming.values_mut() {
        ids.sort_by_key(|id| id.0);
    }

    let mut pending = hydrology
        .segments
        .iter()
        .map(|segment| segment.id)
        .collect::<HashSet<_>>();
    let mut ledgers = HashMap::new();

    while !pending.is_empty() {
        let mut ready = pending
            .iter()
            .copied()
            .filter(|id| {
                let Some(segment) = segments.get(id).copied() else {
                    return true;
                };
                incoming.get(&segment.from).is_none_or(|ids| {
                    ids.iter()
                        .all(|incoming_id| ledgers.contains_key(incoming_id))
                })
            })
            .collect::<Vec<_>>();

        if ready.is_empty() {
            ready = pending.iter().copied().collect();
        }
        ready.sort_by_key(|id| id.0);

        for id in ready {
            if !pending.remove(&id) {
                continue;
            }
            let Some(segment) = segments.get(&id).copied() else {
                continue;
            };
            let incoming_ids = incoming
                .get(&segment.from)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let tributary_flow = incoming_ids
                .iter()
                .filter_map(|incoming_id| ledgers.get(incoming_id))
                .map(|ledger: &SegmentHydraulics| ledger.discharge_q)
                .sum::<f32>();
            let incoming_raw = incoming_ids
                .iter()
                .filter_map(|incoming_id| segments.get(incoming_id).copied())
                .map(|incoming_segment| finite_nonnegative(incoming_segment.raw_flow_accumulation))
                .sum::<f32>();
            let upstream_area =
                (finite_nonnegative(segment.raw_flow_accumulation) - incoming_raw).max(0.0);
            let chain_id = segment_chains
                .get(&id)
                .copied()
                .unwrap_or(RiverChainId(id.0));

            ledgers.insert(
                id,
                SegmentHydraulics::new(chain_id, upstream_area, tributary_flow),
            );
        }
    }

    ledgers
}

fn classify_reach(
    segment: &GraphRiverSegment,
    nodes: &HashMap<GraphDrainageNodeId, GraphDrainageNodeKind>,
    chain_progress: f32,
    config: RiverPlanConfig,
) -> RiverReachType {
    if nodes
        .get(&segment.to)
        .is_some_and(|kind| matches!(kind, GraphDrainageNodeKind::LakeInlet))
    {
        return RiverReachType::LakeInlet;
    }
    if nodes
        .get(&segment.from)
        .is_some_and(|kind| matches!(kind, GraphDrainageNodeKind::LakeOutlet))
    {
        return RiverReachType::LakeOutlet;
    }
    if segment.role == GraphHydrologyRole::Headwater {
        return RiverReachType::Headwater;
    }

    let t = flow_t(segment.flow_accumulation, config);
    if segment.role == GraphHydrologyRole::Trunk && (t >= 0.72 || chain_progress >= 0.72) {
        return RiverReachType::Trunk;
    }
    if t < 0.28 {
        RiverReachType::Upper
    } else if t < 0.52 {
        RiverReachType::Middle
    } else if t < 0.76 {
        RiverReachType::Lower
    } else {
        RiverReachType::Trunk
    }
}

fn segment_morphology(
    segment: &GraphRiverSegment,
    hydraulic: &SegmentHydraulics,
    morphology_q: f32,
    reach_type: RiverReachType,
    chain_progress: f32,
    config: RiverPlanConfig,
) -> SegmentMorphology {
    let display_t = flow_t(segment.flow_accumulation, config);
    let morphology_t = morphology_q_t(morphology_q, config);
    let slope_t = (finite_nonnegative(segment.local_slope) * 70.0).clamp(0.0, 1.0);
    let jitter = signed_jitter(segment.id.0) * 0.08;
    let downstream_t = chain_progress.clamp(0.0, 1.0);
    let hydraulic_width_blocks =
        hydraulic_width_blocks(morphology_q, hydraulic.width_coefficient).max(0.0);
    let hydraulic_depth_blocks =
        hydraulic_depth_blocks(morphology_q, hydraulic.depth_coefficient).max(0.0);
    let bed_width_blocks = match reach_type {
        RiverReachType::Headwater => hydraulic_width_blocks.clamp(1.5, 5.0),
        RiverReachType::Upper => hydraulic_width_blocks.clamp(2.5, 9.0),
        RiverReachType::Middle => hydraulic_width_blocks.clamp(5.0, 24.0),
        RiverReachType::Lower => hydraulic_width_blocks.clamp(18.0, 90.0),
        RiverReachType::Trunk => hydraulic_width_blocks
            .max(lerp(55.0, 105.0, morphology_t.max(downstream_t * 0.55)))
            .clamp(55.0, 165.0),
        RiverReachType::LakeInlet => hydraulic_width_blocks.clamp(3.0, 24.0),
        RiverReachType::LakeOutlet => hydraulic_width_blocks.clamp(5.0, 36.0),
    };
    let bed_depth_blocks = match reach_type {
        RiverReachType::Headwater => hydraulic_depth_blocks.clamp(0.6, 1.8),
        RiverReachType::Upper => hydraulic_depth_blocks.clamp(0.8, 2.8),
        RiverReachType::Middle => hydraulic_depth_blocks.clamp(1.2, 4.8),
        RiverReachType::Lower => hydraulic_depth_blocks.clamp(3.0, 14.0),
        RiverReachType::Trunk => hydraulic_depth_blocks
            .max(lerp(4.5, 8.0, morphology_t.max(downstream_t * 0.55)))
            .clamp(4.5, 22.0),
        RiverReachType::LakeInlet => hydraulic_depth_blocks.clamp(0.8, 3.6),
        RiverReachType::LakeOutlet => hydraulic_depth_blocks.clamp(1.2, 5.2),
    };
    let valley_factor = match reach_type {
        RiverReachType::Headwater => 1.35,
        RiverReachType::Upper => 1.65,
        RiverReachType::Middle => 2.1,
        RiverReachType::Lower => 3.0,
        RiverReachType::Trunk => 3.6,
        RiverReachType::LakeInlet => 1.7,
        RiverReachType::LakeOutlet => 2.1,
    };
    let floodplain_width_blocks = match reach_type {
        RiverReachType::Headwater => lerp(0.0, 0.9, morphology_t),
        RiverReachType::Upper => lerp(0.4, 2.6, morphology_t),
        RiverReachType::Middle => lerp(1.5, 10.0, morphology_t),
        RiverReachType::Lower => lerp(8.0, 56.0, morphology_t),
        RiverReachType::Trunk => lerp(22.0, 145.0, morphology_t.max(downstream_t * 0.45)),
        RiverReachType::LakeInlet => lerp(0.5, 5.0, morphology_t),
        RiverReachType::LakeOutlet => lerp(1.5, 12.0, morphology_t),
    };
    let bank_transition_width_blocks = (bed_width_blocks * lerp(0.22, 0.76, morphology_t)
        + floodplain_width_blocks * 0.08)
        .max(0.65);
    let broad_valley_width_blocks =
        (bed_width_blocks * valley_factor + bank_transition_width_blocks + floodplain_width_blocks)
            * (1.0 + jitter);
    let broad_valley_depth_blocks = match reach_type {
        RiverReachType::Headwater => lerp(0.45, 1.6, morphology_t) + slope_t * 1.2,
        RiverReachType::Upper => lerp(0.8, 2.8, morphology_t) + slope_t * 1.1,
        RiverReachType::Middle => lerp(1.4, 5.2, morphology_t) + slope_t * 0.8,
        RiverReachType::Lower => lerp(4.0, 20.0, morphology_t),
        RiverReachType::Trunk => lerp(8.0, 38.0, morphology_t.max(downstream_t * 0.45)),
        RiverReachType::LakeInlet => lerp(0.8, 3.8, morphology_t),
        RiverReachType::LakeOutlet => lerp(1.2, 5.6, morphology_t),
    };
    let roughness_hint =
        (0.88 - display_t * 0.18 - morphology_t * 0.27 + slope_t * 0.28 + jitter).clamp(0.12, 1.0);
    let gravel_hint =
        (0.72 - display_t * 0.15 - morphology_t * 0.23 + slope_t * 0.36 + jitter * 0.5)
            .clamp(0.0, 1.0);
    let cutbank_hint =
        (0.16 + display_t * 0.18 + morphology_t * 0.37 + slope_t * 0.16 + jitter * 0.5)
            .clamp(0.0, 1.0);

    SegmentMorphology {
        broad_valley_width_blocks: broad_valley_width_blocks.max(bed_width_blocks),
        broad_valley_depth_blocks,
        bed_width_blocks,
        bed_depth_blocks,
        bank_transition_width_blocks,
        floodplain_width_blocks,
        roughness_hint,
        gravel_hint,
        cutbank_hint,
    }
}

fn apply_downstream_depth_continuity(
    chains: &[RiverChain],
    planned_segments: &mut [RiverSegmentPlan],
) {
    let segment_indices = planned_segments
        .iter()
        .enumerate()
        .map(|(index, segment)| (segment.segment_id, index))
        .collect::<HashMap<_, _>>();

    for chain in chains {
        let mut previous_bed_depth = None;
        let mut previous_valley_depth = None;

        for segment_id in &chain.segment_ids {
            let Some(index) = segment_indices.get(segment_id).copied() else {
                continue;
            };
            let segment = &mut planned_segments[index];

            if let Some(previous) = previous_bed_depth {
                segment.bed_depth_blocks = continuous_depth(
                    previous,
                    segment.bed_depth_blocks,
                    bed_depth_max_step(segment.reach_type),
                    if segment.reach_type == RiverReachType::LakeInlet {
                        f32::INFINITY
                    } else {
                        BED_DEPTH_ALLOWED_SHALLOWING_BLOCKS
                    },
                );
            }
            if let Some(previous) = previous_valley_depth {
                segment.broad_valley_depth_blocks = continuous_depth(
                    previous,
                    segment.broad_valley_depth_blocks,
                    broad_valley_depth_max_step(segment.reach_type),
                    if segment.reach_type == RiverReachType::LakeInlet {
                        f32::INFINITY
                    } else {
                        BROAD_VALLEY_DEPTH_ALLOWED_SHALLOWING_BLOCKS
                    },
                );
            }

            previous_bed_depth = Some(segment.bed_depth_blocks);
            previous_valley_depth = Some(segment.broad_valley_depth_blocks);
        }
    }
}

fn continuous_depth(
    previous: f32,
    target: f32,
    max_downstream_increase: f32,
    allowed_shallowing: f32,
) -> f32 {
    let target = finite_nonnegative(target);
    let minimum = if allowed_shallowing.is_finite() {
        (previous - allowed_shallowing).max(0.0)
    } else {
        0.0
    };
    let maximum = previous + max_downstream_increase.max(0.0);
    target.clamp(minimum.min(maximum), maximum)
}

fn bed_depth_max_step(reach_type: RiverReachType) -> f32 {
    match reach_type {
        RiverReachType::Headwater => 1.0,
        RiverReachType::Upper => 1.4,
        RiverReachType::Middle => 2.2,
        RiverReachType::Lower => 3.2,
        RiverReachType::Trunk => 4.0,
        RiverReachType::LakeInlet => 2.0,
        RiverReachType::LakeOutlet => 1.8,
    }
}

fn broad_valley_depth_max_step(reach_type: RiverReachType) -> f32 {
    match reach_type {
        RiverReachType::Headwater => 3.0,
        RiverReachType::Upper => 4.0,
        RiverReachType::Middle => 6.0,
        RiverReachType::Lower => 9.0,
        RiverReachType::Trunk => 12.0,
        RiverReachType::LakeInlet => 5.0,
        RiverReachType::LakeOutlet => 5.0,
    }
}

fn refresh_reach_depths(reaches: &mut [RiverReach], planned_segments: &[RiverSegmentPlan]) {
    let mut depths = HashMap::<RiverReachId, (f32, f32)>::new();
    for segment in planned_segments {
        let entry = depths.entry(segment.reach_id).or_insert((0.0, 0.0));
        entry.0 = entry.0.max(segment.broad_valley_depth_blocks);
        entry.1 = entry.1.max(segment.bed_depth_blocks);
    }

    for reach in reaches {
        if let Some((broad_valley_depth_blocks, bed_depth_blocks)) = depths.get(&reach.id).copied()
        {
            reach.broad_valley_depth_blocks = broad_valley_depth_blocks;
            reach.bed_depth_blocks = bed_depth_blocks;
        }
    }
}

fn smoothed_morphology_discharge_q(
    chain_segments: &[&GraphRiverSegment],
    reach_types: &[RiverReachType],
    hydraulics: &HashMap<GraphRiverSegmentId, SegmentHydraulics>,
    chain_id: RiverChainId,
    config: RiverPlanConfig,
) -> HashMap<GraphRiverSegmentId, f32> {
    let mut smoothed = HashMap::new();
    let mut previous = None;

    for (segment, reach_type) in chain_segments
        .iter()
        .copied()
        .zip(reach_types.iter().copied())
    {
        let hydraulic = hydraulics
            .get(&segment.id)
            .copied()
            .unwrap_or_else(|| SegmentHydraulics::fallback(segment, chain_id));
        let target = base_morphology_discharge_q(segment, &hydraulic, reach_type)
            .max(min_visible_morphology_q(reach_type, config));
        let value = if let Some(previous_q) = previous {
            let maximum =
                previous_q * MORPHOLOGY_Q_MAX_DOWNSTREAM_MULTIPLIER + morphology_q_step(reach_type);
            let minimum: f32 = if matches!(reach_type, RiverReachType::LakeInlet) {
                0.0
            } else {
                previous_q * MORPHOLOGY_Q_ALLOWED_UPSTREAM_FRACTION
            };
            target.clamp(minimum.min(maximum), maximum)
        } else {
            target
        };

        smoothed.insert(segment.id, value);
        previous = Some(value);
    }

    smoothed
}

fn base_morphology_discharge_q(
    segment: &GraphRiverSegment,
    hydraulic: &SegmentHydraulics,
    reach_type: RiverReachType,
) -> f32 {
    let raw_q = finite_nonnegative(hydraulic.discharge_q);
    let display_q = finite_nonnegative(segment.flow_accumulation);
    if matches!(
        reach_type,
        RiverReachType::LakeInlet | RiverReachType::LakeOutlet
    ) && display_q > 0.0
    {
        raw_q.min(display_q)
    } else {
        raw_q
    }
}

fn morphology_q_step(reach_type: RiverReachType) -> f32 {
    let multiplier = match reach_type {
        RiverReachType::Headwater => 0.45,
        RiverReachType::Upper => 0.65,
        RiverReachType::Middle => 0.90,
        RiverReachType::Lower => 1.25,
        RiverReachType::Trunk => 1.60,
        RiverReachType::LakeInlet => 0.60,
        RiverReachType::LakeOutlet => 0.85,
    };
    MORPHOLOGY_Q_MAX_DOWNSTREAM_ADDITION * multiplier
}

fn min_visible_morphology_q(reach_type: RiverReachType, config: RiverPlanConfig) -> f32 {
    let trunk = finite_nonnegative(config.trunk_flow_accumulation).max(1.0);
    match reach_type {
        RiverReachType::Headwater => 3.0,
        RiverReachType::Upper => 6.0,
        RiverReachType::Middle => 14.0,
        RiverReachType::Lower => 32.0,
        RiverReachType::Trunk => trunk * 0.10,
        RiverReachType::LakeInlet => 5.0,
        RiverReachType::LakeOutlet => 8.0,
    }
}

fn flow_t(flow: f32, config: RiverPlanConfig) -> f32 {
    let flow = finite_nonnegative(flow);
    let denom = (config.trunk_flow_accumulation + 1.0)
        .ln()
        .max(f32::EPSILON);
    ((flow + 1.0).ln() / denom).clamp(0.0, 1.0)
}

fn morphology_q_t(discharge_q: f32, config: RiverPlanConfig) -> f32 {
    let trunk = finite_nonnegative(config.trunk_flow_accumulation).max(1.0);
    (finite_nonnegative(discharge_q) / trunk)
        .clamp(0.0, 1.0)
        .sqrt()
}

fn chain_progress(index: usize, len: usize) -> f32 {
    if len <= 1 {
        0.0
    } else {
        index as f32 / (len - 1) as f32
    }
}

fn edge_lengths(patch: &VoronoiGraphPatch) -> HashMap<VoronoiEdgeId, f32> {
    let corners = patch
        .corners
        .iter()
        .map(|corner| (corner.id, corner.position))
        .collect::<HashMap<_, _>>();

    patch
        .edges
        .iter()
        .filter_map(|edge| {
            let a = corners.get(&edge.corners[0]).copied()?;
            let b = corners.get(&edge.corners[1]).copied()?;
            Some((edge.id, distance(a, b)))
        })
        .collect()
}

fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn distance(a: WorldPlanePoint, b: WorldPlanePoint) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

fn signed_jitter(value: u64) -> f32 {
    let mixed = splitmix64(value ^ 0x7247_706c_616e_0001);
    let unit = ((mixed >> 11) as f64 * (1.0 / ((1u64 << 53) as f64))) as f32;
    unit * 2.0 - 1.0
}

fn coefficient_unit(value: u64, salt: u64) -> f32 {
    let mixed = splitmix64(value ^ salt);
    ((mixed >> 11) as f64 * (1.0 / ((1u64 << 53) as f64))) as f32
}

fn chain_coefficients(chain_id: RiverChainId) -> (f32, f32) {
    (
        lerp(
            WIDTH_COEFFICIENT_MIN,
            WIDTH_COEFFICIENT_MAX,
            coefficient_unit(chain_id.0, 0x7269_7665_725f_7769),
        ),
        lerp(
            DEPTH_COEFFICIENT_MIN,
            DEPTH_COEFFICIENT_MAX,
            coefficient_unit(chain_id.0, 0x7269_7665_725f_6465),
        ),
    )
}

fn hydraulic_width_blocks(discharge_q: f32, width_coefficient: f32) -> f32 {
    width_coefficient * finite_nonnegative(discharge_q).sqrt()
}

fn hydraulic_depth_blocks(discharge_q: f32, depth_coefficient: f32) -> f32 {
    depth_coefficient * finite_nonnegative(discharge_q).powf(HYDRAULIC_DEPTH_Q_EXPONENT)
}

fn hydraulic_velocity(discharge_q: f32, width_blocks: f32, depth_blocks: f32) -> f32 {
    let area = width_blocks * depth_blocks;
    if area.is_finite() && area > 0.0 {
        finite_nonnegative(discharge_q) / area
    } else {
        0.0
    }
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SegmentHydraulics {
    upstream_area: f32,
    tributary_flow: f32,
    discharge_q: f32,
    width_coefficient: f32,
    depth_coefficient: f32,
    velocity: f32,
}

impl SegmentHydraulics {
    fn new(chain_id: RiverChainId, upstream_area: f32, tributary_flow: f32) -> Self {
        let upstream_area = finite_nonnegative(upstream_area);
        let tributary_flow = finite_nonnegative(tributary_flow);
        let discharge_q = upstream_area + tributary_flow;
        let (width_coefficient, depth_coefficient) = chain_coefficients(chain_id);
        let width = hydraulic_width_blocks(discharge_q, width_coefficient);
        let depth = hydraulic_depth_blocks(discharge_q, depth_coefficient);

        Self {
            upstream_area,
            tributary_flow,
            discharge_q,
            width_coefficient,
            depth_coefficient,
            velocity: hydraulic_velocity(discharge_q, width, depth),
        }
    }

    fn fallback(segment: &GraphRiverSegment, chain_id: RiverChainId) -> Self {
        Self::new(
            chain_id,
            finite_nonnegative(segment.raw_flow_accumulation),
            0.0,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SegmentMorphology {
    broad_valley_width_blocks: f32,
    broad_valley_depth_blocks: f32,
    bed_width_blocks: f32,
    bed_depth_blocks: f32,
    bank_transition_width_blocks: f32,
    floodplain_width_blocks: f32,
    roughness_hint: f32,
    gravel_hint: f32,
    cutbank_hint: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct ReachBuild {
    id: RiverReachId,
    chain_id: RiverChainId,
    segment_ids: Vec<GraphRiverSegmentId>,
    reach_type: RiverReachType,
    downstream_start: f32,
    downstream_end: f32,
    display_flow: f32,
    raw_flow: f32,
    upstream_area: f32,
    tributary_flow: f32,
    discharge_q: f32,
    morphology_discharge_q: f32,
    hydraulic_width_coefficient: f32,
    hydraulic_depth_coefficient: f32,
    velocity: f32,
    stream_order_hint: f32,
    broad_valley_width_blocks: f32,
    broad_valley_depth_blocks: f32,
    bed_width_blocks: f32,
    bed_depth_blocks: f32,
    bank_transition_width_blocks: f32,
    floodplain_width_blocks: f32,
    roughness_hint: f32,
    gravel_hint: f32,
    cutbank_hint: f32,
}

impl ReachBuild {
    fn new(id: RiverReachId, chain_id: RiverChainId, reach_type: RiverReachType) -> Self {
        Self {
            id,
            chain_id,
            segment_ids: Vec::new(),
            reach_type,
            downstream_start: f32::INFINITY,
            downstream_end: f32::NEG_INFINITY,
            display_flow: 0.0,
            raw_flow: 0.0,
            upstream_area: 0.0,
            tributary_flow: 0.0,
            discharge_q: 0.0,
            morphology_discharge_q: 0.0,
            hydraulic_width_coefficient: 0.0,
            hydraulic_depth_coefficient: 0.0,
            velocity: 0.0,
            stream_order_hint: 0.0,
            broad_valley_width_blocks: 0.0,
            broad_valley_depth_blocks: 0.0,
            bed_width_blocks: 0.0,
            bed_depth_blocks: 0.0,
            bank_transition_width_blocks: 0.0,
            floodplain_width_blocks: 0.0,
            roughness_hint: 0.0,
            gravel_hint: 0.0,
            cutbank_hint: 0.0,
        }
    }

    fn push(
        &mut self,
        segment: &GraphRiverSegment,
        hydraulic: &SegmentHydraulics,
        morphology_discharge_q: f32,
        morphology: &SegmentMorphology,
    ) {
        self.segment_ids.push(segment.id);
        self.downstream_start = self
            .downstream_start
            .min(finite_nonnegative(segment.downstream_progress));
        self.downstream_end = self
            .downstream_end
            .max(finite_nonnegative(segment.downstream_progress));
        self.display_flow = self
            .display_flow
            .max(finite_nonnegative(segment.flow_accumulation));
        self.raw_flow = self
            .raw_flow
            .max(finite_nonnegative(segment.raw_flow_accumulation));
        self.upstream_area = self.upstream_area.max(hydraulic.upstream_area);
        self.tributary_flow = self.tributary_flow.max(hydraulic.tributary_flow);
        self.discharge_q = self.discharge_q.max(hydraulic.discharge_q);
        self.morphology_discharge_q = self
            .morphology_discharge_q
            .max(finite_nonnegative(morphology_discharge_q));
        self.hydraulic_width_coefficient = self
            .hydraulic_width_coefficient
            .max(hydraulic.width_coefficient);
        self.hydraulic_depth_coefficient = self
            .hydraulic_depth_coefficient
            .max(hydraulic.depth_coefficient);
        self.velocity = self.velocity.max(hydraulic.velocity);
        self.stream_order_hint = self
            .stream_order_hint
            .max(flow_t(segment.flow_accumulation, RiverPlanConfig::default()) * 5.0 + 1.0);
        self.broad_valley_width_blocks = self
            .broad_valley_width_blocks
            .max(morphology.broad_valley_width_blocks);
        self.broad_valley_depth_blocks = self
            .broad_valley_depth_blocks
            .max(morphology.broad_valley_depth_blocks);
        self.bed_width_blocks = self.bed_width_blocks.max(morphology.bed_width_blocks);
        self.bed_depth_blocks = self.bed_depth_blocks.max(morphology.bed_depth_blocks);
        self.bank_transition_width_blocks = self
            .bank_transition_width_blocks
            .max(morphology.bank_transition_width_blocks);
        self.floodplain_width_blocks = self
            .floodplain_width_blocks
            .max(morphology.floodplain_width_blocks);
        self.roughness_hint = self.roughness_hint.max(morphology.roughness_hint);
        self.gravel_hint = self.gravel_hint.max(morphology.gravel_hint);
        self.cutbank_hint = self.cutbank_hint.max(morphology.cutbank_hint);
    }

    fn finish(mut self) -> RiverReach {
        if !self.downstream_start.is_finite() {
            self.downstream_start = 0.0;
            self.downstream_end = 0.0;
        }

        RiverReach {
            id: self.id,
            chain_id: self.chain_id,
            segment_ids: self.segment_ids,
            reach_type: self.reach_type,
            downstream_start: self.downstream_start,
            downstream_end: self.downstream_end,
            display_flow: self.display_flow,
            raw_flow: self.raw_flow,
            upstream_area: self.upstream_area,
            tributary_flow: self.tributary_flow,
            discharge_q: self.discharge_q,
            morphology_discharge_q: self.morphology_discharge_q,
            hydraulic_width_coefficient: self.hydraulic_width_coefficient,
            hydraulic_depth_coefficient: self.hydraulic_depth_coefficient,
            velocity: self.velocity,
            stream_order_hint: self.stream_order_hint,
            broad_valley_width_blocks: self.broad_valley_width_blocks,
            broad_valley_depth_blocks: self.broad_valley_depth_blocks,
            bed_width_blocks: self.bed_width_blocks,
            bed_depth_blocks: self.bed_depth_blocks,
            bank_transition_width_blocks: self.bank_transition_width_blocks,
            floodplain_width_blocks: self.floodplain_width_blocks,
            roughness_hint: self.roughness_hint,
            gravel_hint: self.gravel_hint,
            cutbank_hint: self.cutbank_hint,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generation::graph::{
        GraphBaseFields, GraphRegionCoord, VoronoiCorner, VoronoiCornerId, VoronoiEdge,
        VoronoiSiteId,
    };
    use crate::world::generation::hydrology::{
        GraphDrainageNode, GraphHydrologyCorner, GraphHydrologyTopologyStats,
        GraphLocalMinimumResolution, WatershedId,
    };
    use crate::world::generation::macro_map::{MacroEdge, MacroEdgeGuide};

    #[test]
    fn river_plan_preserves_selected_segment_set() {
        let (patch, macro_map, hydrology) = linear_inputs(false);

        let plan = build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());

        let selected = hydrology
            .segments
            .iter()
            .map(|segment| segment.id)
            .collect::<HashSet<_>>();
        let planned = plan
            .segments
            .iter()
            .map(|segment| segment.segment_id)
            .collect::<HashSet<_>>();

        assert_eq!(planned, selected);
        assert_eq!(plan.stats.selected_segment_count, hydrology.segments.len());
        assert_eq!(plan.stats.planned_segment_count, hydrology.segments.len());
        assert_eq!(plan.stats.invalid_lake_edge_segment_count, 0);
    }

    #[test]
    fn morphology_scales_from_headwater_to_trunk() {
        let (patch, macro_map, hydrology) = linear_inputs(false);

        let plan = build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());
        let headwater = plan
            .segment(GraphRiverSegmentId(10))
            .expect("headwater should be planned");
        let middle = plan
            .segment(GraphRiverSegmentId(11))
            .expect("middle segment should be planned");
        let trunk = plan
            .segment(GraphRiverSegmentId(12))
            .expect("trunk should be planned");

        assert_eq!(headwater.reach_type, RiverReachType::Headwater);
        assert!(matches!(
            middle.reach_type,
            RiverReachType::Middle | RiverReachType::Lower
        ));
        assert_eq!(trunk.reach_type, RiverReachType::Trunk);
        assert!(headwater.bed_width_blocks >= 1.5 && headwater.bed_width_blocks <= 5.0);
        assert!(middle.bed_width_blocks >= 5.0 && middle.bed_width_blocks <= 24.0);
        assert!(trunk.bed_width_blocks >= 55.0 && trunk.bed_width_blocks <= 165.0);
        assert!(trunk.bed_width_blocks < 170.0);
        assert!(headwater.bed_depth_blocks <= 1.8);
        assert!(middle.bed_depth_blocks >= 1.2 && middle.bed_depth_blocks <= 4.8);
        assert!(trunk.bed_depth_blocks >= 4.0 && trunk.bed_depth_blocks <= 22.0);
        assert!(headwater.broad_valley_width_blocks < middle.broad_valley_width_blocks);
        assert!(middle.broad_valley_width_blocks < trunk.broad_valley_width_blocks);
        assert!(headwater.floodplain_width_blocks < 0.5);
        assert!(headwater.bank_transition_width_blocks < middle.bank_transition_width_blocks);
        assert!(headwater.roughness_hint > trunk.roughness_hint);
    }

    #[test]
    fn morphology_q_is_smoothed_across_raw_flow_jumps() {
        let (patch, macro_map, hydrology) = linear_inputs(false);

        let plan = build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());
        let trunk = plan
            .segment(GraphRiverSegmentId(12))
            .expect("trunk should be planned");
        let trunk_reach = plan
            .reaches
            .iter()
            .find(|reach| reach.id == trunk.reach_id)
            .expect("trunk reach should be present");
        let unsmoothed =
            morphology_for_test(GraphRiverSegmentId(404), 900.0, RiverReachType::Trunk);

        assert_close(trunk.discharge_q, 900.0);
        assert!(
            trunk_reach.morphology_discharge_q < trunk_reach.discharge_q * 0.20,
            "morphology Q should smooth abrupt selected/pruned joins without changing raw Q"
        );
        assert!(
            trunk.broad_valley_width_blocks < unsmoothed.broad_valley_width_blocks * 0.80,
            "dry-carve valley width should use the smoothed morphology Q"
        );
    }

    #[test]
    fn segment_endpoints_preserve_hydrology_downstream_direction() {
        let (patch, macro_map, hydrology) = linear_inputs(false);

        let plan = build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());
        let endpoints = plan
            .endpoints(GraphRiverSegmentId(10))
            .expect("segment endpoints should be exposed");

        assert_eq!(endpoints.edge, VoronoiEdgeId(20));
        assert_close(endpoints.from_position.x, 0.0);
        assert_close(endpoints.from_position.z, 0.0);
        assert_close(endpoints.to_position.x, 100.0);
        assert_close(endpoints.to_position.z, 0.0);
        assert_eq!(endpoints.downstream_position, endpoints.to_position);
    }

    #[test]
    fn upper_and_middle_widths_are_rescaled_below_lower() {
        let config = RiverPlanConfig::default();
        let upper = morphology_for_test(GraphRiverSegmentId(201), 64.0, RiverReachType::Upper);
        let middle = morphology_for_test(GraphRiverSegmentId(202), 256.0, RiverReachType::Middle);
        let lower = morphology_for_test(GraphRiverSegmentId(203), 1024.0, RiverReachType::Lower);

        assert!(
            middle.bed_width_blocks <= lower.bed_width_blocks * 0.62,
            "middle bed width should read around half of lower"
        );
        assert!(
            upper.bed_width_blocks <= middle.bed_width_blocks * 0.62,
            "upper bed width should read around half of middle"
        );
        assert!(
            middle.broad_valley_width_blocks <= lower.broad_valley_width_blocks * 0.62,
            "middle broad valley should also stay well below lower"
        );
        assert!(
            upper.broad_valley_width_blocks <= middle.broad_valley_width_blocks * 0.70,
            "upper broad valley should stay visibly below middle"
        );
        assert!(flow_t(64.0, config) < flow_t(256.0, config));
    }

    #[test]
    fn downstream_depth_changes_are_bounded() {
        let (patch, macro_map, hydrology) = linear_inputs(false);

        let plan = build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());
        let ordered = [10, 11, 12]
            .into_iter()
            .map(|id| {
                plan.segment(GraphRiverSegmentId(id))
                    .expect("linear segment should be planned")
            })
            .collect::<Vec<_>>();

        for pair in ordered.windows(2) {
            let upstream = pair[0];
            let downstream = pair[1];
            let bed_delta = downstream.bed_depth_blocks - upstream.bed_depth_blocks;
            let valley_delta =
                downstream.broad_valley_depth_blocks - upstream.broad_valley_depth_blocks;

            assert!(bed_delta >= -BED_DEPTH_ALLOWED_SHALLOWING_BLOCKS - 0.0001);
            assert!(bed_delta <= bed_depth_max_step(downstream.reach_type) + 0.0001);
            assert!(valley_delta >= -BROAD_VALLEY_DEPTH_ALLOWED_SHALLOWING_BLOCKS - 0.0001);
            assert!(valley_delta <= broad_valley_depth_max_step(downstream.reach_type) + 0.0001);
        }
    }

    #[test]
    fn river_plan_is_deterministic_for_same_inputs() {
        let (patch, macro_map, hydrology) = linear_inputs(false);

        let first = build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());
        let second = build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());

        assert_eq!(first, second);
    }

    #[test]
    fn raw_q_floor_includes_unselected_drainage_after_pruning() {
        let (patch, macro_map, hydrology) = confluence_inputs();

        let plan = build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());
        let right = plan
            .segment(GraphRiverSegmentId(31))
            .expect("retained strongest headwater should be planned");
        let downstream = plan
            .segment(GraphRiverSegmentId(32))
            .expect("downstream segment should be planned");

        assert_close(right.discharge_q, 26.0);
        assert_close(downstream.tributary_flow, right.discharge_q);
        assert!(
            (downstream.upstream_area - 22.0).abs() <= 0.0001,
            "unselected raw drainage should remain in the local upstream-area floor"
        );
        assert_close(downstream.discharge_q, 48.0);
        assert_close(
            downstream.velocity,
            downstream.discharge_q
                / (downstream.hydraulic_width_coefficient
                    * downstream.discharge_q.sqrt()
                    * downstream.hydraulic_depth_coefficient
                    * downstream.discharge_q.powf(HYDRAULIC_DEPTH_Q_EXPONENT)),
        );
        assert!(downstream.bed_width_blocks > right.bed_width_blocks);
        assert!(downstream.bed_depth_blocks > right.bed_depth_blocks);
    }

    #[test]
    fn hydraulic_depth_is_strongly_q_proportional() {
        let low_q = hydraulic_depth_blocks(16.0, 1.0);
        let high_q = hydraulic_depth_blocks(256.0, 1.0);

        assert!(
            high_q / low_q > 4.0,
            "depth should respond strongly to Q while continuity is handled by the downstream post-pass"
        );
    }

    #[test]
    fn lake_inlet_reach_stays_capacity_limited() {
        let (patch, macro_map, hydrology) = linear_inputs(true);

        let plan = build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());
        let inlet = plan
            .segment(GraphRiverSegmentId(12))
            .expect("terminal lake inlet segment should be planned");

        assert_eq!(inlet.reach_type, RiverReachType::LakeInlet);
        assert!(inlet.bed_width_blocks <= 24.0);
        assert!(inlet.bed_depth_blocks <= 3.6);
    }

    #[test]
    fn selected_lake_edge_segments_are_reported_without_removing_them() {
        let (patch, mut macro_map, hydrology) = linear_inputs(false);
        macro_map.edges[1].lake_class = MacroLakeEdgeClass::LakeBoundary;

        let plan = build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());

        assert_eq!(plan.stats.invalid_lake_edge_segment_count, 1);
        assert!(
            plan.segment(GraphRiverSegmentId(11)).is_some(),
            "river_plan must report invalid selected lake edges but not remove hydrology segments"
        );
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 0.0001,
            "expected {actual} to be close to {expected}"
        );
    }

    fn morphology_for_test(
        id: GraphRiverSegmentId,
        q: f32,
        reach_type: RiverReachType,
    ) -> SegmentMorphology {
        let segment = GraphRiverSegment {
            id,
            edge: VoronoiEdgeId(id.0),
            from: GraphDrainageNodeId(id.0 * 2),
            to: GraphDrainageNodeId(id.0 * 2 + 1),
            watershed: WatershedId(99),
            role: GraphHydrologyRole::Tributary,
            raw_flow_accumulation: q,
            flow_accumulation: q,
            downstream_progress: 0.5,
            local_slope: 0.006,
        };
        let hydraulic = SegmentHydraulics::new(RiverChainId(0), q, 0.0);
        segment_morphology(
            &segment,
            &hydraulic,
            q,
            reach_type,
            0.5,
            RiverPlanConfig::default(),
        )
    }

    fn linear_inputs(
        terminal_lake_inlet: bool,
    ) -> (VoronoiGraphPatch, GraphMacroMap, GraphHydrologyGraph) {
        let patch = VoronoiGraphPatch {
            owner_regions: vec![GraphRegionCoord::new(0, 0)],
            sites: Vec::new(),
            corners: (0..4)
                .map(|index| VoronoiCorner {
                    id: VoronoiCornerId(index),
                    position: WorldPlanePoint::new(index as f32 * 100.0, 0.0),
                    raw_base_fields: GraphBaseFields::default(),
                    base_fields: GraphBaseFields::default(),
                    elevation: 0.3 - index as f32 * 0.05,
                    water_accumulation: 0.0,
                })
                .collect(),
            edges: (0..3)
                .map(|index| VoronoiEdge {
                    id: VoronoiEdgeId(20 + index),
                    sites: [VoronoiSiteId(1), VoronoiSiteId(2)],
                    corners: [VoronoiCornerId(index), VoronoiCornerId(index + 1)],
                    boundary_curve_seed: 0,
                    hydrology_bias: 0.0,
                })
                .collect(),
        };
        let macro_map = GraphMacroMap {
            edges: patch
                .edges
                .iter()
                .map(|edge| MacroEdge {
                    id: edge.id,
                    sites: edge.sites,
                    corners: edge.corners,
                    guide: MacroEdgeGuide {
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
                    },
                    lake_class: MacroLakeEdgeClass::NonLake,
                })
                .collect(),
            ..GraphMacroMap::default()
        };
        let terminal_kind = if terminal_lake_inlet {
            GraphDrainageNodeKind::LakeInlet
        } else {
            GraphDrainageNodeKind::CoastOutlet
        };
        let node_kinds = [
            GraphDrainageNodeKind::Source,
            GraphDrainageNodeKind::Confluence,
            GraphDrainageNodeKind::Confluence,
            terminal_kind,
        ];
        let nodes = node_kinds
            .iter()
            .enumerate()
            .map(|(index, kind)| GraphDrainageNode {
                id: GraphDrainageNodeId(index as u64),
                kind: *kind,
                corner: VoronoiCornerId(index as u64),
                position: WorldPlanePoint::new(index as f32 * 100.0, 0.0),
                watershed: WatershedId(1),
            })
            .collect::<Vec<_>>();
        let corners = (0..4)
            .map(|index| GraphHydrologyCorner {
                id: VoronoiCornerId(index),
                position: WorldPlanePoint::new(index as f32 * 100.0, 0.0),
                elevation: 0.3 - index as f32 * 0.05,
                downstream: (index < 3).then_some(VoronoiCornerId(index + 1)),
                downstream_edge: (index < 3).then_some(VoronoiEdgeId(20 + index)),
                watershed: WatershedId(1),
                flow_accumulation: [12.0, 30.0, 900.0, 900.0][index as usize],
                is_local_minimum: index == 3,
                resolution: if index == 3 {
                    GraphLocalMinimumResolution::OceanOutlet
                } else {
                    GraphLocalMinimumResolution::None
                },
            })
            .collect();
        let segments = [
            (
                10,
                20,
                0,
                1,
                GraphHydrologyRole::Headwater,
                12.0,
                12.0,
                0.0,
                0.025,
            ),
            (
                11,
                21,
                1,
                2,
                GraphHydrologyRole::Tributary,
                30.0,
                30.0,
                0.5,
                0.012,
            ),
            (
                12,
                22,
                2,
                3,
                GraphHydrologyRole::Trunk,
                900.0,
                if terminal_lake_inlet { 28.0 } else { 900.0 },
                1.0,
                0.004,
            ),
        ]
        .into_iter()
        .map(
            |(id, edge, from, to, role, raw, display, progress, slope)| GraphRiverSegment {
                id: GraphRiverSegmentId(id),
                edge: VoronoiEdgeId(edge),
                from: GraphDrainageNodeId(from),
                to: GraphDrainageNodeId(to),
                watershed: WatershedId(1),
                role,
                raw_flow_accumulation: raw,
                flow_accumulation: display,
                downstream_progress: progress,
                local_slope: slope,
            },
        )
        .collect();

        (
            patch,
            macro_map,
            GraphHydrologyGraph {
                corners,
                nodes,
                segments,
                topology_stats: GraphHydrologyTopologyStats::default(),
            },
        )
    }

    fn confluence_inputs() -> (VoronoiGraphPatch, GraphMacroMap, GraphHydrologyGraph) {
        let corner_positions = [
            WorldPlanePoint::new(0.0, -80.0),
            WorldPlanePoint::new(0.0, 80.0),
            WorldPlanePoint::new(100.0, 0.0),
            WorldPlanePoint::new(200.0, 0.0),
        ];
        let patch = VoronoiGraphPatch {
            owner_regions: vec![GraphRegionCoord::new(0, 0)],
            sites: Vec::new(),
            corners: corner_positions
                .iter()
                .enumerate()
                .map(|(index, position)| VoronoiCorner {
                    id: VoronoiCornerId(index as u64),
                    position: *position,
                    raw_base_fields: GraphBaseFields::default(),
                    base_fields: GraphBaseFields::default(),
                    elevation: 0.3 - index as f32 * 0.04,
                    water_accumulation: 0.0,
                })
                .collect(),
            edges: [(40, 0, 2), (41, 1, 2), (42, 2, 3)]
                .into_iter()
                .map(|(edge, from, to)| VoronoiEdge {
                    id: VoronoiEdgeId(edge),
                    sites: [VoronoiSiteId(1), VoronoiSiteId(2)],
                    corners: [VoronoiCornerId(from), VoronoiCornerId(to)],
                    boundary_curve_seed: 0,
                    hydrology_bias: 0.0,
                })
                .collect(),
        };
        let macro_map = GraphMacroMap {
            edges: patch
                .edges
                .iter()
                .map(|edge| MacroEdge {
                    id: edge.id,
                    sites: edge.sites,
                    corners: edge.corners,
                    guide: MacroEdgeGuide {
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
                    },
                    lake_class: MacroLakeEdgeClass::NonLake,
                })
                .collect(),
            ..GraphMacroMap::default()
        };
        let node_kinds = [
            GraphDrainageNodeKind::Source,
            GraphDrainageNodeKind::Source,
            GraphDrainageNodeKind::Source,
            GraphDrainageNodeKind::CoastOutlet,
        ];
        let nodes = node_kinds
            .iter()
            .enumerate()
            .map(|(index, kind)| GraphDrainageNode {
                id: GraphDrainageNodeId(index as u64),
                kind: *kind,
                corner: VoronoiCornerId(index as u64),
                position: corner_positions[index],
                watershed: WatershedId(2),
            })
            .collect::<Vec<_>>();
        let corners = (0..4)
            .map(|index| GraphHydrologyCorner {
                id: VoronoiCornerId(index),
                position: corner_positions[index as usize],
                elevation: 0.3 - index as f32 * 0.04,
                downstream: match index {
                    0 | 1 => Some(VoronoiCornerId(2)),
                    2 => Some(VoronoiCornerId(3)),
                    _ => None,
                },
                downstream_edge: match index {
                    0 => Some(VoronoiEdgeId(40)),
                    1 => Some(VoronoiEdgeId(41)),
                    2 => Some(VoronoiEdgeId(42)),
                    _ => None,
                },
                watershed: WatershedId(2),
                flow_accumulation: [14.0, 26.0, 48.0, 48.0][index as usize],
                is_local_minimum: index == 3,
                resolution: if index == 3 {
                    GraphLocalMinimumResolution::OceanOutlet
                } else {
                    GraphLocalMinimumResolution::None
                },
            })
            .collect();
        let segments = [
            (
                31,
                41,
                1,
                2,
                GraphHydrologyRole::Headwater,
                26.0,
                26.0,
                0.0,
                0.018,
            ),
            (
                32,
                42,
                2,
                3,
                GraphHydrologyRole::Tributary,
                48.0,
                48.0,
                1.0,
                0.008,
            ),
        ]
        .into_iter()
        .map(
            |(id, edge, from, to, role, raw, display, progress, slope)| GraphRiverSegment {
                id: GraphRiverSegmentId(id),
                edge: VoronoiEdgeId(edge),
                from: GraphDrainageNodeId(from),
                to: GraphDrainageNodeId(to),
                watershed: WatershedId(2),
                role,
                raw_flow_accumulation: raw,
                flow_accumulation: display,
                downstream_progress: progress,
                local_slope: slope,
            },
        )
        .collect();

        (
            patch,
            macro_map,
            GraphHydrologyGraph {
                corners,
                nodes,
                segments,
                topology_stats: GraphHydrologyTopologyStats::default(),
            },
        )
    }
}
