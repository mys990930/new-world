use std::collections::{HashMap, HashSet};

use super::graph::{VoronoiEdgeId, VoronoiGraphPatch};
use super::hydrology::{
    GraphDrainageNodeId, GraphDrainageNodeKind, GraphHydrologyGraph, GraphHydrologyRole,
    GraphRiverSegment, GraphRiverSegmentId,
};
use super::macro_map::GraphMacroMap;

const FLOW_HINT_NORMALIZER: f32 = 32.0;
const RAW_FLOW_HINT_NORMALIZER: f32 = 48.0;
const LAKE_BOUND_BROAD_WIDTH_CAP_BLOCKS: f32 = 132.0;
const LAKE_BOUND_BED_WIDTH_CAP_BLOCKS: f32 = 30.0;
const LAKE_BOUND_BED_DEPTH_CAP: f32 = 0.30;

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

impl RiverReachType {
    pub const fn as_index(self) -> usize {
        match self {
            Self::Headwater => 0,
            Self::Upper => 1,
            Self::Middle => 2,
            Self::Lower => 3,
            Self::Trunk => 4,
            Self::LakeInlet => 5,
            Self::LakeOutlet => 6,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Headwater => "headwater",
            Self::Upper => "upper",
            Self::Middle => "middle",
            Self::Lower => "lower",
            Self::Trunk => "trunk",
            Self::LakeInlet => "lake_inlet",
            Self::LakeOutlet => "lake_outlet",
        }
    }

    pub const fn all() -> [Self; 7] {
        [
            Self::Headwater,
            Self::Upper,
            Self::Middle,
            Self::Lower,
            Self::Trunk,
            Self::LakeInlet,
            Self::LakeOutlet,
        ]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RiverPlan {
    pub chains: Vec<RiverChain>,
    pub reaches: Vec<RiverReach>,
    pub segment_plans: Vec<RiverSegmentPlan>,
    pub stats: RiverPlanStats,
}

impl Default for RiverPlan {
    fn default() -> Self {
        Self {
            chains: Vec::new(),
            reaches: Vec::new(),
            segment_plans: Vec::new(),
            stats: RiverPlanStats::default(),
        }
    }
}

impl RiverPlan {
    pub fn is_empty(&self) -> bool {
        self.segment_plans.is_empty()
    }

    pub fn segment_plan(&self, id: GraphRiverSegmentId) -> Option<&RiverSegmentPlan> {
        self.segment_plans.iter().find(|plan| plan.segment == id)
    }

    pub fn plans_for_edge(&self, edge: VoronoiEdgeId) -> impl Iterator<Item = &RiverSegmentPlan> {
        self.segment_plans
            .iter()
            .filter(move |plan| plan.edge == edge)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RiverChain {
    pub id: RiverChainId,
    pub segment_ids: Vec<GraphRiverSegmentId>,
    pub terminal_kind: Option<GraphDrainageNodeKind>,
    pub display_flow: f32,
    pub raw_flow: f32,
    pub downstream_start: f32,
    pub downstream_end: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RiverReach {
    pub id: RiverReachId,
    pub chain: RiverChainId,
    pub segment_ids: Vec<GraphRiverSegmentId>,
    pub reach_type: RiverReachType,
    pub downstream_start: f32,
    pub downstream_end: f32,
    pub display_flow: f32,
    pub raw_flow: f32,
    pub stream_order_hint: f32,
    pub broad_valley_width_blocks: f32,
    pub broad_valley_depth: f32,
    pub bed_width_blocks: f32,
    pub bed_depth: f32,
    pub bank_transition_width_blocks: f32,
    pub floodplain_width_blocks: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiverSegmentPlan {
    pub segment: GraphRiverSegmentId,
    pub edge: VoronoiEdgeId,
    pub chain: RiverChainId,
    pub reach: RiverReachId,
    pub reach_type: RiverReachType,
    pub downstream_progress: f32,
    pub display_flow: f32,
    pub raw_flow: f32,
    pub stream_order_hint: f32,
    pub broad_valley_width_blocks: f32,
    pub broad_valley_depth: f32,
    pub bed_width_blocks: f32,
    pub bed_depth: f32,
    pub bank_transition_width_blocks: f32,
    pub floodplain_width_blocks: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RiverPlanStats {
    pub selected_segment_count: usize,
    pub chain_count: usize,
    pub reach_count: usize,
    pub reach_type_counts: [usize; 7],
    pub min_broad_valley_width_blocks: f32,
    pub max_broad_valley_width_blocks: f32,
    pub min_broad_valley_depth: f32,
    pub max_broad_valley_depth: f32,
    pub min_bed_width_blocks: f32,
    pub max_bed_width_blocks: f32,
    pub min_bed_depth: f32,
    pub max_bed_depth: f32,
}

pub fn generate_river_plan(
    _patch: &VoronoiGraphPatch,
    _macro_map: &GraphMacroMap,
    hydrology: &GraphHydrologyGraph,
) -> RiverPlan {
    let node_kinds = hydrology
        .nodes
        .iter()
        .map(|node| (node.id, node.kind))
        .collect::<HashMap<_, _>>();
    let segment_by_id = hydrology
        .segments
        .iter()
        .map(|segment| (segment.id, segment))
        .collect::<HashMap<_, _>>();
    let outgoing_by_node = outgoing_segments_by_node(&hydrology.segments);
    let incoming_counts = incoming_segment_counts(&hydrology.segments);
    let mut starts = hydrology
        .segments
        .iter()
        .filter(|segment| {
            incoming_counts.get(&segment.from).copied().unwrap_or(0) == 0
                || node_kinds
                    .get(&segment.from)
                    .is_some_and(|kind| *kind == GraphDrainageNodeKind::LakeOutlet)
        })
        .map(|segment| segment.id)
        .collect::<Vec<_>>();
    starts.sort_by_key(|id| id.0);
    starts.dedup();

    let mut visited = HashSet::new();
    let mut chains = Vec::new();
    for start in starts {
        if !visited.contains(&start) {
            chains.push(build_chain(
                chains.len(),
                start,
                &segment_by_id,
                &outgoing_by_node,
                &incoming_counts,
                &node_kinds,
                &mut visited,
            ));
        }
    }

    for segment in &hydrology.segments {
        if !visited.contains(&segment.id) {
            chains.push(build_chain(
                chains.len(),
                segment.id,
                &segment_by_id,
                &outgoing_by_node,
                &incoming_counts,
                &node_kinds,
                &mut visited,
            ));
        }
    }

    let mut reaches = Vec::new();
    let mut segment_plans = Vec::with_capacity(hydrology.segments.len());
    for chain in &chains {
        let chain_segments = chain
            .segment_ids
            .iter()
            .filter_map(|id| segment_by_id.get(id).copied())
            .cloned()
            .collect::<Vec<_>>();
        append_chain_reaches(
            chain,
            &chain_segments,
            &node_kinds,
            &mut reaches,
            &mut segment_plans,
        );
    }

    segment_plans.sort_by_key(|plan| plan.segment.0);
    reaches.sort_by_key(|reach| reach.id.0);
    chains.sort_by_key(|chain| chain.id.0);

    let stats = river_plan_stats(&segment_plans, chains.len(), reaches.len());

    RiverPlan {
        chains,
        reaches,
        segment_plans,
        stats,
    }
}

fn outgoing_segments_by_node(
    segments: &[GraphRiverSegment],
) -> HashMap<GraphDrainageNodeId, Vec<GraphRiverSegmentId>> {
    let mut outgoing = HashMap::<GraphDrainageNodeId, Vec<GraphRiverSegmentId>>::new();
    for segment in segments {
        outgoing.entry(segment.from).or_default().push(segment.id);
    }
    for ids in outgoing.values_mut() {
        ids.sort_by_key(|id| id.0);
    }
    outgoing
}

fn incoming_segment_counts(segments: &[GraphRiverSegment]) -> HashMap<GraphDrainageNodeId, usize> {
    let mut incoming = HashMap::<GraphDrainageNodeId, usize>::new();
    for segment in segments {
        *incoming.entry(segment.to).or_default() += 1;
    }
    incoming
}

fn build_chain(
    chain_index: usize,
    start: GraphRiverSegmentId,
    segment_by_id: &HashMap<GraphRiverSegmentId, &GraphRiverSegment>,
    outgoing_by_node: &HashMap<GraphDrainageNodeId, Vec<GraphRiverSegmentId>>,
    incoming_counts: &HashMap<GraphDrainageNodeId, usize>,
    node_kinds: &HashMap<GraphDrainageNodeId, GraphDrainageNodeKind>,
    visited: &mut HashSet<GraphRiverSegmentId>,
) -> RiverChain {
    let id = RiverChainId(chain_index as u64);
    let mut segment_ids = Vec::new();
    let mut current = Some(start);
    while let Some(segment_id) = current {
        if !visited.insert(segment_id) {
            break;
        }
        let Some(segment) = segment_by_id.get(&segment_id).copied() else {
            break;
        };
        segment_ids.push(segment_id);

        if node_kinds
            .get(&segment.to)
            .is_some_and(|kind| is_chain_terminal_kind(*kind))
        {
            break;
        }
        if incoming_counts.get(&segment.to).copied().unwrap_or(0) > 1 {
            break;
        }
        let next = outgoing_by_node
            .get(&segment.to)
            .and_then(|ids| ids.first().copied());
        if next.is_none() {
            break;
        }
        current = next;
    }

    let display_flow = segment_ids
        .iter()
        .filter_map(|id| segment_by_id.get(id))
        .map(|segment| segment.flow_accumulation)
        .fold(0.0, f32::max);
    let raw_flow = segment_ids
        .iter()
        .filter_map(|id| segment_by_id.get(id))
        .map(|segment| segment.raw_flow_accumulation)
        .fold(0.0, f32::max);
    let downstream_start = segment_ids
        .iter()
        .filter_map(|id| segment_by_id.get(id))
        .map(|segment| segment.downstream_progress)
        .fold(f32::INFINITY, f32::min);
    let downstream_end = segment_ids
        .iter()
        .filter_map(|id| segment_by_id.get(id))
        .map(|segment| segment.downstream_progress)
        .fold(f32::NEG_INFINITY, f32::max);
    let terminal_kind = segment_ids
        .last()
        .and_then(|id| segment_by_id.get(id).copied())
        .and_then(|segment| node_kinds.get(&segment.to).copied());

    RiverChain {
        id,
        segment_ids,
        terminal_kind,
        display_flow,
        raw_flow,
        downstream_start: finite_or_zero(downstream_start),
        downstream_end: finite_or_zero(downstream_end),
    }
}

fn append_chain_reaches(
    chain: &RiverChain,
    segments: &[GraphRiverSegment],
    node_kinds: &HashMap<GraphDrainageNodeId, GraphDrainageNodeKind>,
    reaches: &mut Vec<RiverReach>,
    segment_plans: &mut Vec<RiverSegmentPlan>,
) {
    let progress_min = segments
        .iter()
        .map(|segment| segment.downstream_progress)
        .fold(f32::INFINITY, f32::min);
    let progress_max = segments
        .iter()
        .map(|segment| segment.downstream_progress)
        .fold(f32::NEG_INFINITY, f32::max);
    let progress_span = (progress_max - progress_min).max(f32::EPSILON);

    let mut current_reach_type = None;
    let mut current_segments = Vec::<RiverSegmentPlan>::new();
    for segment in segments {
        let normalized_progress =
            ((segment.downstream_progress - progress_min) / progress_span).clamp(0.0, 1.0);
        let reach_type = classify_reach(segment, normalized_progress, node_kinds);
        let metrics = morphology_for_segment(segment, reach_type, normalized_progress);
        let plan = RiverSegmentPlan {
            segment: segment.id,
            edge: segment.edge,
            chain: chain.id,
            reach: RiverReachId(reaches.len() as u64),
            reach_type,
            downstream_progress: normalized_progress,
            display_flow: segment.flow_accumulation,
            raw_flow: segment.raw_flow_accumulation,
            stream_order_hint: metrics.stream_order_hint,
            broad_valley_width_blocks: metrics.broad_valley_width_blocks,
            broad_valley_depth: metrics.broad_valley_depth,
            bed_width_blocks: metrics.bed_width_blocks,
            bed_depth: metrics.bed_depth,
            bank_transition_width_blocks: metrics.bank_transition_width_blocks,
            floodplain_width_blocks: metrics.floodplain_width_blocks,
        };

        if current_reach_type == Some(reach_type) || current_segments.is_empty() {
            current_reach_type = Some(reach_type);
            current_segments.push(plan);
        } else {
            push_reach(chain.id, reaches, segment_plans, &mut current_segments);
            current_reach_type = Some(reach_type);
            current_segments.push(plan);
        }
    }
    if !current_segments.is_empty() {
        push_reach(chain.id, reaches, segment_plans, &mut current_segments);
    }
}

fn push_reach(
    chain: RiverChainId,
    reaches: &mut Vec<RiverReach>,
    segment_plans: &mut Vec<RiverSegmentPlan>,
    plans: &mut Vec<RiverSegmentPlan>,
) {
    let id = RiverReachId(reaches.len() as u64);
    for plan in plans.iter_mut() {
        plan.reach = id;
    }
    let reach_type = plans[0].reach_type;
    let downstream_start = plans
        .iter()
        .map(|plan| plan.downstream_progress)
        .fold(f32::INFINITY, f32::min);
    let downstream_end = plans
        .iter()
        .map(|plan| plan.downstream_progress)
        .fold(f32::NEG_INFINITY, f32::max);
    let display_flow = plans
        .iter()
        .map(|plan| plan.display_flow)
        .fold(0.0, f32::max);
    let raw_flow = plans.iter().map(|plan| plan.raw_flow).fold(0.0, f32::max);
    let stream_order_hint = plans
        .iter()
        .map(|plan| plan.stream_order_hint)
        .fold(0.0, f32::max);
    let broad_valley_width_blocks = plans
        .iter()
        .map(|plan| plan.broad_valley_width_blocks)
        .fold(0.0, f32::max);
    let broad_valley_depth = plans
        .iter()
        .map(|plan| plan.broad_valley_depth)
        .fold(0.0, f32::max);
    let bed_width_blocks = plans
        .iter()
        .map(|plan| plan.bed_width_blocks)
        .fold(0.0, f32::max);
    let bed_depth = plans.iter().map(|plan| plan.bed_depth).fold(0.0, f32::max);
    let bank_transition_width_blocks = plans
        .iter()
        .map(|plan| plan.bank_transition_width_blocks)
        .fold(0.0, f32::max);
    let floodplain_width_blocks = plans
        .iter()
        .map(|plan| plan.floodplain_width_blocks)
        .fold(0.0, f32::max);
    let segment_ids = plans.iter().map(|plan| plan.segment).collect::<Vec<_>>();

    reaches.push(RiverReach {
        id,
        chain,
        segment_ids,
        reach_type,
        downstream_start: finite_or_zero(downstream_start),
        downstream_end: finite_or_zero(downstream_end),
        display_flow,
        raw_flow,
        stream_order_hint,
        broad_valley_width_blocks,
        broad_valley_depth,
        bed_width_blocks,
        bed_depth,
        bank_transition_width_blocks,
        floodplain_width_blocks,
    });
    segment_plans.extend(plans.drain(..));
}

fn classify_reach(
    segment: &GraphRiverSegment,
    downstream_progress: f32,
    node_kinds: &HashMap<GraphDrainageNodeId, GraphDrainageNodeKind>,
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

    let display_hint = flow_hint(segment.flow_accumulation, FLOW_HINT_NORMALIZER);
    let raw_hint = flow_hint(segment.raw_flow_accumulation, RAW_FLOW_HINT_NORMALIZER);
    let maturity =
        (display_hint * 0.58 + raw_hint * 0.18 + downstream_progress * 0.24).clamp(0.0, 1.0);

    if matches!(
        segment.role,
        GraphHydrologyRole::Trunk | GraphHydrologyRole::Floodplain
    ) && (maturity >= 0.58 || downstream_progress >= 0.62)
    {
        RiverReachType::Trunk
    } else if maturity >= 0.70 || downstream_progress >= 0.82 {
        RiverReachType::Lower
    } else if maturity >= 0.44 || downstream_progress >= 0.55 {
        RiverReachType::Middle
    } else if maturity >= 0.20 || display_hint >= 0.16 {
        RiverReachType::Upper
    } else {
        RiverReachType::Headwater
    }
}

fn morphology_for_segment(
    segment: &GraphRiverSegment,
    reach_type: RiverReachType,
    downstream_progress: f32,
) -> MorphologyMetrics {
    let display_hint = flow_hint(segment.flow_accumulation, FLOW_HINT_NORMALIZER);
    let raw_hint = flow_hint(segment.raw_flow_accumulation, RAW_FLOW_HINT_NORMALIZER);
    let maturity =
        (display_hint * 0.68 + downstream_progress * 0.24 + raw_hint * 0.08).clamp(0.0, 1.0);
    let order_hint = (display_hint * 0.78 + raw_hint * 0.22).clamp(0.0, 1.0);
    let (base_width, width_span, base_depth, depth_span, base_bed, bed_span) = match reach_type {
        RiverReachType::Headwater => (34.0, 28.0, 0.08, 0.08, 4.0, 6.0),
        RiverReachType::Upper => (58.0, 42.0, 0.12, 0.12, 8.0, 10.0),
        RiverReachType::Middle => (94.0, 74.0, 0.18, 0.17, 14.0, 18.0),
        RiverReachType::Lower => (146.0, 118.0, 0.28, 0.22, 24.0, 28.0),
        RiverReachType::Trunk => (210.0, 170.0, 0.36, 0.27, 38.0, 40.0),
        RiverReachType::LakeInlet => (62.0, 64.0, 0.12, 0.12, 9.0, 18.0),
        RiverReachType::LakeOutlet => (72.0, 68.0, 0.13, 0.13, 11.0, 20.0),
    };

    let width_t = maturity.powf(1.08);
    let depth_t = maturity.powf(1.02);
    let mut broad_width = base_width + width_span * width_t;
    let mut broad_depth = base_depth + depth_span * depth_t;
    let mut bed_width = base_bed + bed_span * display_hint.powf(1.05);
    let mut bed_depth = (broad_depth * 0.78 + display_hint * 0.18).clamp(0.06, 0.72);

    if matches!(
        reach_type,
        RiverReachType::LakeInlet | RiverReachType::LakeOutlet
    ) {
        broad_width = broad_width.min(LAKE_BOUND_BROAD_WIDTH_CAP_BLOCKS);
        bed_width = bed_width.min(LAKE_BOUND_BED_WIDTH_CAP_BLOCKS);
        bed_depth = bed_depth.min(LAKE_BOUND_BED_DEPTH_CAP);
        broad_depth = broad_depth.min(0.26);
    }

    MorphologyMetrics {
        stream_order_hint: order_hint,
        broad_valley_width_blocks: broad_width,
        broad_valley_depth: broad_depth,
        bed_width_blocks: bed_width,
        bed_depth,
        bank_transition_width_blocks: (broad_width * 0.20 + bed_width * 0.35).max(8.0),
        floodplain_width_blocks: match reach_type {
            RiverReachType::Lower | RiverReachType::Trunk => broad_width * (0.28 + maturity * 0.24),
            RiverReachType::LakeInlet | RiverReachType::LakeOutlet => broad_width * 0.16,
            _ => broad_width * 0.10,
        },
    }
}

fn river_plan_stats(
    segment_plans: &[RiverSegmentPlan],
    chain_count: usize,
    reach_count: usize,
) -> RiverPlanStats {
    if segment_plans.is_empty() {
        return RiverPlanStats {
            chain_count,
            reach_count,
            ..RiverPlanStats::default()
        };
    }

    let mut stats = RiverPlanStats {
        selected_segment_count: segment_plans.len(),
        chain_count,
        reach_count,
        min_broad_valley_width_blocks: f32::INFINITY,
        max_broad_valley_width_blocks: f32::NEG_INFINITY,
        min_broad_valley_depth: f32::INFINITY,
        max_broad_valley_depth: f32::NEG_INFINITY,
        min_bed_width_blocks: f32::INFINITY,
        max_bed_width_blocks: f32::NEG_INFINITY,
        min_bed_depth: f32::INFINITY,
        max_bed_depth: f32::NEG_INFINITY,
        ..RiverPlanStats::default()
    };

    for plan in segment_plans {
        stats.reach_type_counts[plan.reach_type.as_index()] += 1;
        stats.min_broad_valley_width_blocks = stats
            .min_broad_valley_width_blocks
            .min(plan.broad_valley_width_blocks);
        stats.max_broad_valley_width_blocks = stats
            .max_broad_valley_width_blocks
            .max(plan.broad_valley_width_blocks);
        stats.min_broad_valley_depth = stats.min_broad_valley_depth.min(plan.broad_valley_depth);
        stats.max_broad_valley_depth = stats.max_broad_valley_depth.max(plan.broad_valley_depth);
        stats.min_bed_width_blocks = stats.min_bed_width_blocks.min(plan.bed_width_blocks);
        stats.max_bed_width_blocks = stats.max_bed_width_blocks.max(plan.bed_width_blocks);
        stats.min_bed_depth = stats.min_bed_depth.min(plan.bed_depth);
        stats.max_bed_depth = stats.max_bed_depth.max(plan.bed_depth);
    }

    stats
}

fn is_chain_terminal_kind(kind: GraphDrainageNodeKind) -> bool {
    matches!(
        kind,
        GraphDrainageNodeKind::LakeInlet
            | GraphDrainageNodeKind::Sink
            | GraphDrainageNodeKind::CoastOutlet
    )
}

fn flow_hint(flow: f32, normalizer: f32) -> f32 {
    (flow.max(0.0).sqrt() / normalizer.max(f32::EPSILON)).clamp(0.0, 1.0)
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

#[derive(Debug, Clone, Copy)]
struct MorphologyMetrics {
    stream_order_hint: f32,
    broad_valley_width_blocks: f32,
    broad_valley_depth: f32,
    bed_width_blocks: f32,
    bed_depth: f32,
    bank_transition_width_blocks: f32,
    floodplain_width_blocks: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generation::graph::{VoronoiEdgeId, WorldPlanePoint};
    use crate::world::generation::hydrology::{
        GraphDrainageNode, GraphHydrologyGraph, WatershedId,
    };

    #[test]
    fn river_plan_preserves_selected_segment_count() {
        let hydrology = synthetic_hydrology(false);

        let plan = generate_river_plan(
            &VoronoiGraphPatch::default(),
            &GraphMacroMap::default(),
            &hydrology,
        );

        assert_eq!(plan.segment_plans.len(), hydrology.segments.len());
        assert_eq!(plan.stats.selected_segment_count, hydrology.segments.len());
    }

    #[test]
    fn downstream_trunk_width_increases() {
        let hydrology = synthetic_hydrology(false);

        let plan = generate_river_plan(
            &VoronoiGraphPatch::default(),
            &GraphMacroMap::default(),
            &hydrology,
        );
        let upstream = plan
            .segment_plan(GraphRiverSegmentId(10))
            .expect("upstream segment should be planned");
        let downstream = plan
            .segment_plan(GraphRiverSegmentId(12))
            .expect("downstream segment should be planned");

        assert!(downstream.broad_valley_width_blocks > upstream.broad_valley_width_blocks);
        assert!(downstream.bed_width_blocks > upstream.bed_width_blocks);
        assert!(downstream.bed_depth > upstream.bed_depth);
    }

    #[test]
    fn lake_inlet_morphology_stays_capped() {
        let lake_hydrology = synthetic_hydrology(true);
        let ocean_hydrology = synthetic_hydrology(false);

        let lake_plan = generate_river_plan(
            &VoronoiGraphPatch::default(),
            &GraphMacroMap::default(),
            &lake_hydrology,
        );
        let ocean_plan = generate_river_plan(
            &VoronoiGraphPatch::default(),
            &GraphMacroMap::default(),
            &ocean_hydrology,
        );
        let lake_inlet = lake_plan
            .segment_plan(GraphRiverSegmentId(12))
            .expect("lake inlet segment should be planned");
        let ocean_trunk = ocean_plan
            .segment_plan(GraphRiverSegmentId(12))
            .expect("ocean trunk segment should be planned");

        assert_eq!(lake_inlet.reach_type, RiverReachType::LakeInlet);
        assert!(lake_inlet.broad_valley_width_blocks <= LAKE_BOUND_BROAD_WIDTH_CAP_BLOCKS);
        assert!(lake_inlet.bed_width_blocks <= LAKE_BOUND_BED_WIDTH_CAP_BLOCKS);
        assert!(lake_inlet.bed_depth <= LAKE_BOUND_BED_DEPTH_CAP);
        assert!(lake_inlet.broad_valley_width_blocks < ocean_trunk.broad_valley_width_blocks);
    }

    fn synthetic_hydrology(lake_terminal: bool) -> GraphHydrologyGraph {
        let terminal_kind = if lake_terminal {
            GraphDrainageNodeKind::LakeInlet
        } else {
            GraphDrainageNodeKind::CoastOutlet
        };
        let nodes = vec![
            node(1, GraphDrainageNodeKind::Source),
            node(2, GraphDrainageNodeKind::Confluence),
            node(3, GraphDrainageNodeKind::Confluence),
            node(4, terminal_kind),
        ];
        let terminal_flow = if lake_terminal { 32.0 } else { 900.0 };
        GraphHydrologyGraph {
            nodes,
            segments: vec![
                segment(10, 1, 2, 18.0, 18.0, 0.0, GraphHydrologyRole::Headwater),
                segment(11, 2, 3, 160.0, 160.0, 0.5, GraphHydrologyRole::Tributary),
                segment(
                    12,
                    3,
                    4,
                    terminal_flow,
                    900.0,
                    1.0,
                    GraphHydrologyRole::Trunk,
                ),
            ],
            ..GraphHydrologyGraph::default()
        }
    }

    fn node(id: u64, kind: GraphDrainageNodeKind) -> GraphDrainageNode {
        GraphDrainageNode {
            id: GraphDrainageNodeId(id),
            kind,
            corner: crate::world::generation::graph::VoronoiCornerId(id),
            position: WorldPlanePoint::new(id as f32, 0.0),
            watershed: WatershedId(1),
        }
    }

    fn segment(
        id: u64,
        from: u64,
        to: u64,
        flow: f32,
        raw_flow: f32,
        downstream_progress: f32,
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
            downstream_progress,
        }
    }
}
