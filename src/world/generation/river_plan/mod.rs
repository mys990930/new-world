use std::collections::{HashMap, HashSet};

use super::graph::{VoronoiEdgeId, VoronoiGraphPatch, WorldPlanePoint};
use super::hydrology::{
    GraphDrainageNodeId, GraphDrainageNodeKind, GraphHydrologyGraph, GraphRiverSegment,
    GraphRiverSegmentId,
};
use super::macro_map::{GraphMacroMap, MacroLakeEdgeClass};

pub const DEFAULT_RIVER_PLAN_TRUNK_FLOW: f32 = 1024.0;

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
            .find(|endpoint| endpoint.segment_id == id)
    }
}

pub fn build_river_plan(
    patch: &VoronoiGraphPatch,
    macro_map: &GraphMacroMap,
    hydrology: &GraphHydrologyGraph,
    config: RiverPlanConfig,
) -> RiverPlan {
    validate_config(config);

    let hydrology_segments = hydrology
        .segments
        .iter()
        .map(|segment| (segment.id, segment))
        .collect::<HashMap<_, _>>();
    let node_kinds = hydrology
        .nodes
        .iter()
        .map(|node| (node.id, node.kind))
        .collect::<HashMap<_, _>>();
    let node_positions = hydrology
        .nodes
        .iter()
        .map(|node| (node.id, node.position))
        .collect::<HashMap<_, _>>();
    let lake_edges = macro_map
        .edges
        .iter()
        .filter(|edge| edge.lake_class != MacroLakeEdgeClass::NonLake)
        .map(|edge| edge.id)
        .collect::<Vec<_>>();

    let topology = SelectedRiverTopology::new(&hydrology.segments);
    let chain_segment_ids = build_chains(&topology, &node_kinds);
    let mut segment_plans = Vec::with_capacity(hydrology.segments.len());
    let mut chains = Vec::with_capacity(chain_segment_ids.len());
    let mut reaches = Vec::new();
    let mut next_reach_id = 0u64;

    for (chain_index, chain_ids) in chain_segment_ids.iter().enumerate() {
        let chain_id = RiverChainId(chain_index as u64);
        let chain_length = chain_ids
            .iter()
            .filter_map(|id| hydrology_segments.get(id).copied())
            .map(|segment| segment_length(segment, &node_positions))
            .sum::<f32>();
        let mut length_so_far = 0.0;
        let mut raw_ledger = 0.0f32;
        let mut display_ledger = 0.0f32;
        let coefficient_seed = chain_id.0 ^ chain_ids.first().map(|id| id.0).unwrap_or_default();
        let width_coefficient = 0.78 + stable_unit(coefficient_seed ^ 0x9e37_79b9) * 0.28;
        let depth_coefficient = 0.46 + stable_unit(coefficient_seed ^ 0x85eb_ca6b) * 0.20;
        let mut chain_plans = Vec::with_capacity(chain_ids.len());

        for segment_id in chain_ids {
            let Some(segment) = hydrology_segments.get(segment_id).copied() else {
                continue;
            };
            let segment_length_blocks = segment_length(segment, &node_positions);
            let midpoint_progress = if chain_length > f32::EPSILON {
                ((length_so_far + segment_length_blocks * 0.5) / chain_length).clamp(0.0, 1.0)
            } else {
                segment.downstream_progress.clamp(0.0, 1.0)
            };
            length_so_far += segment_length_blocks;

            let display_flow = segment.flow_accumulation.max(0.0);
            let raw_flow = segment.raw_flow_accumulation.max(display_flow);
            raw_ledger = raw_ledger.max(raw_flow);
            display_ledger = display_ledger.max(display_flow);

            let reach_type =
                reach_type_for_segment(segment, &node_kinds, midpoint_progress, config);
            let morphology = morphology_for_segment(
                segment,
                reach_type,
                display_ledger,
                width_coefficient,
                depth_coefficient,
                config,
            );

            chain_plans.push(RiverSegmentPlan {
                segment_id: segment.id,
                edge: segment.edge,
                chain_id,
                reach_id: RiverReachId(0),
                reach_type,
                raw_flow,
                display_flow,
                upstream_area: raw_ledger,
                tributary_flow: (raw_ledger - display_ledger).max(0.0),
                discharge_q: display_ledger,
                hydraulic_width_coefficient: morphology.width_coefficient,
                hydraulic_depth_coefficient: morphology.depth_coefficient,
                velocity: morphology.velocity,
                downstream_progress: segment.downstream_progress.clamp(0.0, 1.0),
                chain_downstream_progress: midpoint_progress,
                segment_length_blocks,
                local_slope: segment.local_slope.max(0.0),
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
        }

        let chain_reaches = build_reaches_for_chain(chain_id, &mut chain_plans, &mut next_reach_id);
        let terminal_kind = chain_ids
            .last()
            .and_then(|segment_id| hydrology_segments.get(segment_id).copied())
            .and_then(|segment| node_kinds.get(&segment.to).copied());
        let min_downstream_progress = chain_plans
            .iter()
            .map(|segment| segment.chain_downstream_progress)
            .fold(f32::INFINITY, f32::min);
        let max_downstream_progress = chain_plans
            .iter()
            .map(|segment| segment.chain_downstream_progress)
            .fold(f32::NEG_INFINITY, f32::max);

        chains.push(RiverChain {
            id: chain_id,
            segment_ids: chain_ids.clone(),
            terminal_kind,
            length_blocks: chain_length,
            min_downstream_progress: finite_or_zero(min_downstream_progress),
            max_downstream_progress: finite_or_zero(max_downstream_progress),
        });
        reaches.extend(chain_reaches);
        segment_plans.extend(chain_plans);
    }

    let mut segments = segment_plans;
    segments.sort_by_key(|segment| segment.segment_id.0);

    let mut segment_endpoints = hydrology
        .segments
        .iter()
        .filter_map(|segment| {
            let from_position = node_positions.get(&segment.from).copied()?;
            let to_position = node_positions.get(&segment.to).copied()?;
            Some(RiverSegmentEndpointPlan {
                segment_id: segment.id,
                edge: segment.edge,
                from_position,
                to_position,
                downstream_position: to_position,
            })
        })
        .collect::<Vec<_>>();
    segment_endpoints.sort_by_key(|endpoint| endpoint.segment_id.0);

    RiverPlan {
        stats: RiverPlanStats {
            selected_segment_count: hydrology.segments.len(),
            planned_segment_count: segments.len(),
            chain_count: chains.len(),
            reach_count: reaches.len(),
            invalid_lake_edge_segment_count: segments
                .iter()
                .filter(|segment| lake_edges.contains(&segment.edge))
                .count(),
            missing_patch_edge_count: segments
                .iter()
                .filter(|segment| !patch.edges.iter().any(|edge| edge.id == segment.edge))
                .count(),
        },
        chains,
        reaches,
        segments,
        segment_endpoints,
    }
}

#[derive(Debug, Clone)]
struct SelectedRiverTopology<'a> {
    segments: Vec<&'a GraphRiverSegment>,
    by_id: HashMap<GraphRiverSegmentId, &'a GraphRiverSegment>,
    outgoing: HashMap<GraphDrainageNodeId, Vec<GraphRiverSegmentId>>,
    incoming: HashMap<GraphDrainageNodeId, Vec<GraphRiverSegmentId>>,
}

impl<'a> SelectedRiverTopology<'a> {
    fn new(segments: &'a [GraphRiverSegment]) -> Self {
        let mut ordered_segments = segments.iter().collect::<Vec<_>>();
        ordered_segments.sort_by_key(|segment| segment.id.0);

        let mut by_id = HashMap::new();
        let mut outgoing = HashMap::<GraphDrainageNodeId, Vec<GraphRiverSegmentId>>::new();
        let mut incoming = HashMap::<GraphDrainageNodeId, Vec<GraphRiverSegmentId>>::new();

        for segment in &ordered_segments {
            by_id.insert(segment.id, *segment);
            outgoing.entry(segment.from).or_default().push(segment.id);
            incoming.entry(segment.to).or_default().push(segment.id);
        }

        for ids in outgoing.values_mut() {
            ids.sort_by_key(|id| id.0);
        }
        for ids in incoming.values_mut() {
            ids.sort_by_key(|id| id.0);
        }

        Self {
            segments: ordered_segments,
            by_id,
            outgoing,
            incoming,
        }
    }

    fn segment(&self, id: GraphRiverSegmentId) -> Option<&'a GraphRiverSegment> {
        self.by_id.get(&id).copied()
    }

    fn outgoing_ids(&self, node: GraphDrainageNodeId) -> &[GraphRiverSegmentId] {
        self.outgoing.get(&node).map(Vec::as_slice).unwrap_or(&[])
    }

    fn incoming_count(&self, node: GraphDrainageNodeId) -> usize {
        self.incoming.get(&node).map(Vec::len).unwrap_or_default()
    }

    fn outgoing_count(&self, node: GraphDrainageNodeId) -> usize {
        self.outgoing.get(&node).map(Vec::len).unwrap_or_default()
    }
}

fn build_chains(
    topology: &SelectedRiverTopology<'_>,
    node_kinds: &HashMap<GraphDrainageNodeId, GraphDrainageNodeKind>,
) -> Vec<Vec<GraphRiverSegmentId>> {
    let mut visited = HashSet::new();
    let mut chains = Vec::new();

    for segment in &topology.segments {
        if visited.contains(&segment.id) || !is_chain_start(segment, topology, node_kinds) {
            continue;
        }
        chains.push(walk_chain(segment.id, topology, node_kinds, &mut visited));
    }

    for segment in &topology.segments {
        if visited.contains(&segment.id) {
            continue;
        }
        chains.push(walk_chain(segment.id, topology, node_kinds, &mut visited));
    }

    chains
}

fn is_chain_start(
    segment: &GraphRiverSegment,
    topology: &SelectedRiverTopology<'_>,
    node_kinds: &HashMap<GraphDrainageNodeId, GraphDrainageNodeKind>,
) -> bool {
    let from_kind = node_kinds.get(&segment.from).copied();
    matches!(
        from_kind,
        Some(GraphDrainageNodeKind::Source)
            | Some(GraphDrainageNodeKind::LakeOutlet)
            | Some(GraphDrainageNodeKind::Confluence)
    ) || topology.incoming_count(segment.from) != 1
        || topology.outgoing_count(segment.from) != 1
}

fn walk_chain(
    start: GraphRiverSegmentId,
    topology: &SelectedRiverTopology<'_>,
    node_kinds: &HashMap<GraphDrainageNodeId, GraphDrainageNodeKind>,
    visited: &mut HashSet<GraphRiverSegmentId>,
) -> Vec<GraphRiverSegmentId> {
    let mut chain = Vec::new();
    let mut current = start;

    loop {
        if !visited.insert(current) {
            break;
        }
        chain.push(current);

        let Some(segment) = topology.segment(current) else {
            break;
        };
        if is_chain_end(segment.to, topology, node_kinds) {
            break;
        }
        let outgoing = topology.outgoing_ids(segment.to);
        if outgoing.len() != 1 || visited.contains(&outgoing[0]) {
            break;
        }
        current = outgoing[0];
    }

    chain
}

fn is_chain_end(
    node: GraphDrainageNodeId,
    topology: &SelectedRiverTopology<'_>,
    node_kinds: &HashMap<GraphDrainageNodeId, GraphDrainageNodeKind>,
) -> bool {
    if !matches!(
        node_kinds.get(&node).copied(),
        None | Some(GraphDrainageNodeKind::Source)
    ) {
        return true;
    }

    topology.incoming_count(node) != 1 || topology.outgoing_count(node) != 1
}

fn build_reaches_for_chain(
    chain_id: RiverChainId,
    segments: &mut [RiverSegmentPlan],
    next_reach_id: &mut u64,
) -> Vec<RiverReach> {
    let mut reaches = Vec::new();
    let mut start = 0usize;

    while start < segments.len() {
        let reach_type = segments[start].reach_type;
        let mut end = start + 1;
        while end < segments.len() && segments[end].reach_type == reach_type {
            end += 1;
        }

        let reach_id = RiverReachId(*next_reach_id);
        *next_reach_id += 1;
        for segment in &mut segments[start..end] {
            segment.reach_id = reach_id;
        }

        let slice = &segments[start..end];
        reaches.push(RiverReach {
            id: reach_id,
            chain_id,
            segment_ids: slice.iter().map(|segment| segment.segment_id).collect(),
            reach_type,
            downstream_start: slice
                .first()
                .map(|segment| segment.chain_downstream_progress)
                .unwrap_or_default(),
            downstream_end: slice
                .last()
                .map(|segment| segment.chain_downstream_progress)
                .unwrap_or_default(),
            display_flow: slice
                .iter()
                .map(|segment| segment.display_flow)
                .fold(0.0, f32::max),
            raw_flow: slice
                .iter()
                .map(|segment| segment.raw_flow)
                .fold(0.0, f32::max),
            upstream_area: slice
                .iter()
                .map(|segment| segment.upstream_area)
                .fold(0.0, f32::max),
            tributary_flow: slice
                .iter()
                .map(|segment| segment.tributary_flow)
                .fold(0.0, f32::max),
            discharge_q: slice
                .iter()
                .map(|segment| segment.discharge_q)
                .fold(0.0, f32::max),
            morphology_discharge_q: slice
                .iter()
                .map(|segment| segment.discharge_q)
                .fold(0.0, f32::max),
            hydraulic_width_coefficient: average_by(slice, |segment| {
                segment.hydraulic_width_coefficient
            }),
            hydraulic_depth_coefficient: average_by(slice, |segment| {
                segment.hydraulic_depth_coefficient
            }),
            velocity: average_by(slice, |segment| segment.velocity),
            stream_order_hint: slice
                .iter()
                .map(|segment| stream_order_hint(segment.discharge_q))
                .fold(0.0, f32::max),
            broad_valley_width_blocks: slice
                .iter()
                .map(|segment| segment.broad_valley_width_blocks)
                .fold(0.0, f32::max),
            broad_valley_depth_blocks: slice
                .iter()
                .map(|segment| segment.broad_valley_depth_blocks)
                .fold(0.0, f32::max),
            bed_width_blocks: slice
                .iter()
                .map(|segment| segment.bed_width_blocks)
                .fold(0.0, f32::max),
            bed_depth_blocks: slice
                .iter()
                .map(|segment| segment.bed_depth_blocks)
                .fold(0.0, f32::max),
            bank_transition_width_blocks: slice
                .iter()
                .map(|segment| segment.bank_transition_width_blocks)
                .fold(0.0, f32::max),
            floodplain_width_blocks: slice
                .iter()
                .map(|segment| segment.floodplain_width_blocks)
                .fold(0.0, f32::max),
            roughness_hint: average_by(slice, |segment| segment.roughness_hint),
            gravel_hint: average_by(slice, |segment| segment.gravel_hint),
            cutbank_hint: slice
                .iter()
                .map(|segment| segment.cutbank_hint)
                .fold(0.0, f32::max),
        });

        start = end;
    }

    reaches
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SegmentMorphology {
    width_coefficient: f32,
    depth_coefficient: f32,
    velocity: f32,
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

fn validate_config(config: RiverPlanConfig) {
    assert!(
        config.trunk_flow_accumulation.is_finite() && config.trunk_flow_accumulation > 0.0,
        "river plan trunk flow must be finite and positive"
    );
}

fn reach_type_for_segment(
    segment: &GraphRiverSegment,
    node_kinds: &HashMap<GraphDrainageNodeId, GraphDrainageNodeKind>,
    chain_progress: f32,
    config: RiverPlanConfig,
) -> RiverReachType {
    if node_kinds
        .get(&segment.to)
        .is_some_and(|kind| *kind == GraphDrainageNodeKind::LakeInlet)
    {
        return RiverReachType::LakeInlet;
    }
    if node_kinds
        .get(&segment.from)
        .is_some_and(|kind| *kind == GraphDrainageNodeKind::LakeOutlet)
    {
        return RiverReachType::LakeOutlet;
    }
    let ratio = flow_ratio(segment.flow_accumulation, config).max(chain_progress * 0.06);
    if ratio >= 0.70 {
        RiverReachType::Trunk
    } else if ratio >= 0.30 {
        RiverReachType::Lower
    } else if ratio >= 0.10 {
        RiverReachType::Middle
    } else if ratio >= 0.035 {
        RiverReachType::Upper
    } else {
        RiverReachType::Headwater
    }
}

fn morphology_for_segment(
    segment: &GraphRiverSegment,
    reach_type: RiverReachType,
    morphology_discharge_q: f32,
    width_coefficient: f32,
    depth_coefficient: f32,
    config: RiverPlanConfig,
) -> SegmentMorphology {
    let ratio = flow_ratio(morphology_discharge_q, config);
    let q = morphology_discharge_q.max(1.0);
    let hydraulic_width = width_coefficient * q.sqrt();
    let hydraulic_depth = depth_coefficient * q.powf(0.4);
    let (min_bed_width, max_bed_width, min_bed_depth, max_bed_depth, reach_min) = match reach_type {
        RiverReachType::Headwater => (1.5, 5.0, 0.6, 1.8, 0.0),
        RiverReachType::Upper => (2.5, 9.0, 0.8, 2.8, 0.04),
        RiverReachType::Middle => (5.0, 24.0, 1.2, 4.8, 0.12),
        RiverReachType::Lower => (18.0, 96.0, 3.0, 14.0, 0.28),
        RiverReachType::Trunk => (24.0, 165.0, 4.0, 22.0, 0.45),
        RiverReachType::LakeInlet => (2.5, 16.0, 0.8, 4.0, 0.04),
        RiverReachType::LakeOutlet => (4.0, 28.0, 1.0, 5.0, 0.08),
    };
    let conservative_lake_scale = match reach_type {
        RiverReachType::LakeInlet | RiverReachType::LakeOutlet => 0.72,
        _ => 1.0,
    };
    let scaled = ratio.max(reach_min).clamp(0.0, 1.0);
    let bed_width_blocks =
        (hydraulic_width * conservative_lake_scale).clamp(min_bed_width, max_bed_width);
    let bed_depth_blocks =
        (hydraulic_depth * conservative_lake_scale).clamp(min_bed_depth, max_bed_depth);

    SegmentMorphology {
        width_coefficient,
        depth_coefficient,
        velocity: (q / (hydraulic_width * hydraulic_depth).max(1.0)).max(0.0),
        broad_valley_width_blocks: (bed_width_blocks * (2.8 + scaled * 4.2))
            .clamp(min_bed_width * 3.0, 420.0),
        broad_valley_depth_blocks: (bed_depth_blocks * (1.35 + scaled * 2.3))
            .clamp(min_bed_depth, 72.0),
        bed_width_blocks,
        bed_depth_blocks,
        bank_transition_width_blocks: (bed_width_blocks * (0.7 + scaled)).clamp(2.0, 96.0),
        floodplain_width_blocks: (bed_width_blocks * scaled * 2.4).clamp(0.0, 240.0),
        roughness_hint: (1.0 - scaled * 0.75).clamp(0.12, 1.0),
        gravel_hint: (0.65 - scaled * 0.25 + segment.local_slope * 8.0).clamp(0.0, 1.0),
        cutbank_hint: (segment.local_slope * 20.0 + scaled * 0.35).clamp(0.0, 1.0),
    }
}

fn segment_length(
    segment: &GraphRiverSegment,
    node_positions: &HashMap<GraphDrainageNodeId, WorldPlanePoint>,
) -> f32 {
    let Some(from) = node_positions.get(&segment.from) else {
        return 0.0;
    };
    let Some(to) = node_positions.get(&segment.to) else {
        return 0.0;
    };
    ((to.x - from.x).powi(2) + (to.z - from.z).powi(2)).sqrt()
}

fn flow_ratio(flow: f32, config: RiverPlanConfig) -> f32 {
    (flow.max(0.0) / config.trunk_flow_accumulation).clamp(0.0, 1.0)
}

fn stream_order_hint(discharge_q: f32) -> f32 {
    (discharge_q.max(0.0).ln_1p() / DEFAULT_RIVER_PLAN_TRUNK_FLOW.ln_1p()).clamp(0.0, 1.0)
}

fn average_by<T>(items: &[T], value: impl Fn(&T) -> f32) -> f32 {
    if items.is_empty() {
        return 0.0;
    }
    items.iter().map(value).sum::<f32>() / items.len() as f32
}

fn finite_or_zero(value: f32) -> f32 {
    value.is_finite().then_some(value).unwrap_or_default()
}

fn stable_unit(mut value: u64) -> f32 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    value = value.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    value ^= value >> 33;
    (value as f64 / u64::MAX as f64) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generation::graph::{
        GraphBaseFields, GraphRegionCoord, VoronoiCorner, VoronoiCornerId, VoronoiEdge,
        VoronoiSiteId,
    };
    use crate::world::generation::hydrology::{GraphDrainageNode, GraphHydrologyRole, WatershedId};
    use crate::world::generation::macro_map::{MacroEdge, MacroEdgeGuide};

    #[test]
    fn selected_hydrology_segments_are_preserved() {
        let (patch, macro_map, hydrology) = synthetic_inputs(
            &[
                segment(1, 0, 1, 40.0, GraphHydrologyRole::Tributary),
                segment(2, 1, 2, 42.0, GraphHydrologyRole::Tributary),
                segment(3, 2, 3, 44.0, GraphHydrologyRole::Tributary),
            ],
            &[
                (0, GraphDrainageNodeKind::Source),
                (3, GraphDrainageNodeKind::CoastOutlet),
            ],
            &[],
        );

        let plan = build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());

        assert_eq!(plan.stats.selected_segment_count, hydrology.segments.len());
        assert_eq!(plan.stats.planned_segment_count, hydrology.segments.len());
        for source in &hydrology.segments {
            assert!(plan.segment(source.id).is_some());
            assert!(plan.endpoints(source.id).is_some());
        }
    }

    #[test]
    fn adjacent_selected_segments_form_chain_and_coherent_reach() {
        let (patch, macro_map, hydrology) = synthetic_inputs(
            &[
                segment(1, 0, 1, 40.0, GraphHydrologyRole::Tributary),
                segment(2, 1, 2, 42.0, GraphHydrologyRole::Tributary),
                segment(3, 2, 3, 44.0, GraphHydrologyRole::Tributary),
            ],
            &[
                (0, GraphDrainageNodeKind::Source),
                (3, GraphDrainageNodeKind::CoastOutlet),
            ],
            &[],
        );

        let plan = build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());

        assert_eq!(plan.chains.len(), 1);
        assert_eq!(
            plan.chains[0].segment_ids,
            vec![
                GraphRiverSegmentId(1),
                GraphRiverSegmentId(2),
                GraphRiverSegmentId(3)
            ]
        );
        assert_eq!(plan.reaches.len(), 1);
        assert_eq!(plan.reaches[0].segment_ids.len(), 3);
    }

    #[test]
    fn confluence_splits_chain_structure_deterministically() {
        let (patch, macro_map, hydrology) = synthetic_inputs(
            &[
                segment(10, 0, 2, 60.0, GraphHydrologyRole::Tributary),
                segment(11, 1, 2, 55.0, GraphHydrologyRole::Tributary),
                segment(12, 2, 3, 120.0, GraphHydrologyRole::Trunk),
            ],
            &[
                (0, GraphDrainageNodeKind::Source),
                (1, GraphDrainageNodeKind::Source),
                (2, GraphDrainageNodeKind::Confluence),
                (3, GraphDrainageNodeKind::CoastOutlet),
            ],
            &[],
        );

        let plan = build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());
        let chains = plan
            .chains
            .iter()
            .map(|chain| chain.segment_ids.clone())
            .collect::<Vec<_>>();

        assert_eq!(
            chains,
            vec![
                vec![GraphRiverSegmentId(10)],
                vec![GraphRiverSegmentId(11)],
                vec![GraphRiverSegmentId(12)]
            ]
        );
    }

    #[test]
    fn ordinary_chain_discharge_and_morphology_are_nondecreasing() {
        let (patch, macro_map, hydrology) = synthetic_inputs(
            &[
                segment(1, 0, 1, 8.0, GraphHydrologyRole::Headwater),
                segment(2, 1, 2, 6.0, GraphHydrologyRole::Headwater),
                segment(3, 2, 3, 20.0, GraphHydrologyRole::Tributary),
            ],
            &[
                (0, GraphDrainageNodeKind::Source),
                (3, GraphDrainageNodeKind::CoastOutlet),
            ],
            &[],
        );

        let plan = build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());
        let ordered = plan.chains[0]
            .segment_ids
            .iter()
            .map(|id| plan.segment(*id).expect("segment"))
            .collect::<Vec<_>>();

        assert!(ordered.windows(2).all(|pair| {
            pair[1].discharge_q >= pair[0].discharge_q
                && pair[1].bed_width_blocks >= pair[0].bed_width_blocks
                && pair[1].bed_depth_blocks >= pair[0].bed_depth_blocks
                && pair[1].broad_valley_width_blocks >= pair[0].broad_valley_width_blocks
        }));
    }

    #[test]
    fn reach_morphology_scales_from_headwater_to_trunk() {
        let (patch, macro_map, hydrology) = synthetic_inputs(
            &[
                segment(1, 0, 1, 4.0, GraphHydrologyRole::Headwater),
                segment(2, 2, 3, 60.0, GraphHydrologyRole::Tributary),
                segment(3, 4, 5, 160.0, GraphHydrologyRole::Tributary),
                segment(4, 6, 7, 500.0, GraphHydrologyRole::Floodplain),
                segment(5, 8, 9, 900.0, GraphHydrologyRole::Trunk),
            ],
            &[
                (1, GraphDrainageNodeKind::CoastOutlet),
                (3, GraphDrainageNodeKind::CoastOutlet),
                (5, GraphDrainageNodeKind::CoastOutlet),
                (7, GraphDrainageNodeKind::CoastOutlet),
                (9, GraphDrainageNodeKind::CoastOutlet),
            ],
            &[],
        );

        let plan = build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());
        let headwater = plan.segment(GraphRiverSegmentId(1)).expect("headwater");
        let upper = plan.segment(GraphRiverSegmentId(2)).expect("upper");
        let middle = plan.segment(GraphRiverSegmentId(3)).expect("middle");
        let floodplain = plan.segment(GraphRiverSegmentId(4)).expect("floodplain");
        let trunk = plan.segment(GraphRiverSegmentId(5)).expect("trunk");

        assert_eq!(headwater.reach_type, RiverReachType::Headwater);
        assert_eq!(upper.reach_type, RiverReachType::Upper);
        assert_eq!(middle.reach_type, RiverReachType::Middle);
        assert_eq!(floodplain.reach_type, RiverReachType::Lower);
        assert_eq!(trunk.reach_type, RiverReachType::Trunk);
        assert!(headwater.bed_width_blocks < upper.bed_width_blocks);
        assert!(upper.bed_width_blocks < middle.bed_width_blocks);
        assert!(middle.bed_width_blocks < floodplain.bed_width_blocks);
        assert!(floodplain.bed_width_blocks <= trunk.bed_width_blocks);
    }

    #[test]
    fn morphology_scale_follows_q_more_than_hydrology_role_label() {
        let (patch, macro_map, hydrology) = synthetic_inputs(
            &[
                segment(1, 0, 1, 120.0, GraphHydrologyRole::Trunk),
                segment(2, 2, 3, 500.0, GraphHydrologyRole::Floodplain),
            ],
            &[
                (1, GraphDrainageNodeKind::CoastOutlet),
                (3, GraphDrainageNodeKind::CoastOutlet),
            ],
            &[],
        );

        let plan = build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());
        let trunk = plan.segment(GraphRiverSegmentId(1)).expect("trunk");
        let floodplain = plan.segment(GraphRiverSegmentId(2)).expect("floodplain");

        assert_eq!(trunk.reach_type, RiverReachType::Middle);
        assert_eq!(floodplain.reach_type, RiverReachType::Lower);
        assert!(floodplain.discharge_q >= trunk.discharge_q);
        assert!(floodplain.bed_width_blocks >= trunk.bed_width_blocks);
        assert!(floodplain.broad_valley_width_blocks >= trunk.broad_valley_width_blocks);
    }

    #[test]
    fn lake_inlet_keeps_system_q_with_conservative_morphology() {
        let (patch, macro_map, hydrology) = synthetic_inputs(
            &[
                segment_with_raw(1, 0, 1, 900.0, 900.0, GraphHydrologyRole::Floodplain),
                segment(2, 2, 3, 900.0, GraphHydrologyRole::Trunk),
            ],
            &[
                (1, GraphDrainageNodeKind::LakeInlet),
                (3, GraphDrainageNodeKind::CoastOutlet),
            ],
            &[],
        );

        let plan = build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());
        let inlet = plan.segment(GraphRiverSegmentId(1)).expect("lake inlet");
        let trunk = plan.segment(GraphRiverSegmentId(2)).expect("trunk");

        assert_eq!(inlet.reach_type, RiverReachType::LakeInlet);
        assert_eq!(inlet.raw_flow, 900.0);
        assert_eq!(inlet.display_flow, 900.0);
        assert_eq!(inlet.discharge_q, 900.0);
        assert!(inlet.bed_width_blocks < trunk.bed_width_blocks);
        assert!(inlet.broad_valley_width_blocks < trunk.broad_valley_width_blocks);
    }

    #[test]
    fn system_q_does_not_decrease_across_lake_chain_boundary() {
        let (patch, macro_map, hydrology) = synthetic_inputs(
            &[
                segment(1, 0, 1, 180.0, GraphHydrologyRole::Floodplain),
                segment(2, 2, 3, 180.0, GraphHydrologyRole::Floodplain),
                segment(3, 3, 4, 220.0, GraphHydrologyRole::Floodplain),
            ],
            &[
                (1, GraphDrainageNodeKind::LakeInlet),
                (2, GraphDrainageNodeKind::LakeOutlet),
                (4, GraphDrainageNodeKind::CoastOutlet),
            ],
            &[],
        );

        let plan = build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());
        let inlet = plan.segment(GraphRiverSegmentId(1)).expect("lake inlet");
        let outlet = plan.segment(GraphRiverSegmentId(2)).expect("lake outlet");
        let downstream = plan.segment(GraphRiverSegmentId(3)).expect("downstream");

        assert_eq!(inlet.reach_type, RiverReachType::LakeInlet);
        assert_eq!(outlet.reach_type, RiverReachType::LakeOutlet);
        assert!(outlet.discharge_q >= inlet.discharge_q);
        assert!(downstream.discharge_q >= outlet.discharge_q);
    }

    fn segment(
        id: u64,
        from: u64,
        to: u64,
        flow: f32,
        role: GraphHydrologyRole,
    ) -> GraphRiverSegment {
        segment_with_raw(id, from, to, flow, flow, role)
    }

    fn segment_with_raw(
        id: u64,
        from: u64,
        to: u64,
        flow: f32,
        raw_flow: f32,
        role: GraphHydrologyRole,
    ) -> GraphRiverSegment {
        GraphRiverSegment {
            id: GraphRiverSegmentId(id),
            edge: VoronoiEdgeId(id),
            from: GraphDrainageNodeId(from),
            to: GraphDrainageNodeId(to),
            watershed: WatershedId(1),
            role,
            raw_flow_accumulation: raw_flow,
            flow_accumulation: flow,
            downstream_progress: flow / DEFAULT_RIVER_PLAN_TRUNK_FLOW,
            local_slope: 0.01,
        }
    }

    fn synthetic_inputs(
        segments: &[GraphRiverSegment],
        node_kind_overrides: &[(u64, GraphDrainageNodeKind)],
        lake_edges: &[VoronoiEdgeId],
    ) -> (VoronoiGraphPatch, GraphMacroMap, GraphHydrologyGraph) {
        let mut node_ids = segments
            .iter()
            .flat_map(|segment| [segment.from, segment.to])
            .collect::<Vec<_>>();
        node_ids.sort_by_key(|id| id.0);
        node_ids.dedup();

        let nodes = node_ids
            .iter()
            .map(|id| {
                let kind = node_kind_overrides
                    .iter()
                    .find(|(node, _)| GraphDrainageNodeId(*node) == *id)
                    .map(|(_, kind)| *kind)
                    .unwrap_or(GraphDrainageNodeKind::Source);
                GraphDrainageNode {
                    id: *id,
                    kind,
                    corner: VoronoiCornerId(id.0),
                    position: WorldPlanePoint::new(id.0 as f32 * 64.0, 0.0),
                    watershed: WatershedId(1),
                }
            })
            .collect::<Vec<_>>();

        let corners = node_ids
            .iter()
            .map(|id| VoronoiCorner {
                id: VoronoiCornerId(id.0),
                position: WorldPlanePoint::new(id.0 as f32 * 64.0, 0.0),
                raw_base_fields: GraphBaseFields::default(),
                base_fields: GraphBaseFields::default(),
                elevation: 0.0,
                water_accumulation: 0.0,
            })
            .collect::<Vec<_>>();
        let patch_edges = segments
            .iter()
            .map(|segment| VoronoiEdge {
                id: segment.edge,
                sites: [VoronoiSiteId(1), VoronoiSiteId(2)],
                corners: [
                    VoronoiCornerId(segment.from.0),
                    VoronoiCornerId(segment.to.0),
                ],
                boundary_curve_seed: segment.id.0,
                hydrology_bias: 0.0,
            })
            .collect::<Vec<_>>();
        let macro_edges = segments
            .iter()
            .map(|segment| MacroEdge {
                id: segment.edge,
                sites: [VoronoiSiteId(1), VoronoiSiteId(2)],
                corners: [
                    VoronoiCornerId(segment.from.0),
                    VoronoiCornerId(segment.to.0),
                ],
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
                lake_class: if lake_edges.contains(&segment.edge) {
                    MacroLakeEdgeClass::LakeBoundary
                } else {
                    MacroLakeEdgeClass::NonLake
                },
            })
            .collect::<Vec<_>>();

        (
            VoronoiGraphPatch {
                owner_regions: vec![GraphRegionCoord::new(0, 0)],
                sites: Vec::new(),
                corners,
                edges: patch_edges,
            },
            GraphMacroMap {
                sites: Vec::new(),
                corners: Vec::new(),
                edges: macro_edges,
                biomes: Vec::new(),
            },
            GraphHydrologyGraph {
                corners: Vec::new(),
                nodes,
                segments: segments.to_vec(),
                topology_stats: Default::default(),
            },
        )
    }
}
