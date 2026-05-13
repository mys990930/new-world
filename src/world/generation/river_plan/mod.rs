use std::collections::HashMap;

use super::graph::{VoronoiEdgeId, VoronoiGraphPatch, WorldPlanePoint};
use super::hydrology::{
    GraphDrainageNodeId, GraphDrainageNodeKind, GraphHydrologyGraph, GraphHydrologyRole,
    GraphRiverSegment, GraphRiverSegmentId,
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

    let mut segments = hydrology
        .segments
        .iter()
        .enumerate()
        .map(|(index, segment)| {
            let chain_id = RiverChainId(index as u64);
            let reach_id = RiverReachId(index as u64);
            let reach_type = reach_type_for_segment(segment, &node_kinds, config);
            let display_flow = segment.flow_accumulation.max(0.0);
            let raw_flow = segment.raw_flow_accumulation.max(display_flow);
            let morphology = morphology_for_segment(segment, reach_type, config);

            RiverSegmentPlan {
                segment_id: segment.id,
                edge: segment.edge,
                chain_id,
                reach_id,
                reach_type,
                raw_flow,
                display_flow,
                upstream_area: raw_flow,
                tributary_flow: 0.0,
                discharge_q: raw_flow,
                hydraulic_width_coefficient: morphology.width_coefficient,
                hydraulic_depth_coefficient: morphology.depth_coefficient,
                velocity: morphology.velocity,
                downstream_progress: segment.downstream_progress.clamp(0.0, 1.0),
                chain_downstream_progress: segment.downstream_progress.clamp(0.0, 1.0),
                segment_length_blocks: segment_length(segment, &node_positions),
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
            }
        })
        .collect::<Vec<_>>();
    segments.sort_by_key(|segment| segment.segment_id.0);

    let segment_endpoints = hydrology
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

    let chains = segments
        .iter()
        .map(|segment| RiverChain {
            id: segment.chain_id,
            segment_ids: vec![segment.segment_id],
            terminal_kind: hydrology
                .segments
                .iter()
                .find(|source| source.id == segment.segment_id)
                .and_then(|source| node_kinds.get(&source.to).copied()),
            length_blocks: segment.segment_length_blocks,
            min_downstream_progress: segment.chain_downstream_progress,
            max_downstream_progress: segment.chain_downstream_progress,
        })
        .collect::<Vec<_>>();

    let reaches = segments
        .iter()
        .map(|segment| RiverReach {
            id: segment.reach_id,
            chain_id: segment.chain_id,
            segment_ids: vec![segment.segment_id],
            reach_type: segment.reach_type,
            downstream_start: segment.chain_downstream_progress,
            downstream_end: segment.chain_downstream_progress,
            display_flow: segment.display_flow,
            raw_flow: segment.raw_flow,
            upstream_area: segment.upstream_area,
            tributary_flow: segment.tributary_flow,
            discharge_q: segment.discharge_q,
            morphology_discharge_q: segment.discharge_q,
            hydraulic_width_coefficient: segment.hydraulic_width_coefficient,
            hydraulic_depth_coefficient: segment.hydraulic_depth_coefficient,
            velocity: segment.velocity,
            stream_order_hint: flow_ratio(segment.display_flow, config),
            broad_valley_width_blocks: segment.broad_valley_width_blocks,
            broad_valley_depth_blocks: segment.broad_valley_depth_blocks,
            bed_width_blocks: segment.bed_width_blocks,
            bed_depth_blocks: segment.bed_depth_blocks,
            bank_transition_width_blocks: segment.bank_transition_width_blocks,
            floodplain_width_blocks: segment.floodplain_width_blocks,
            roughness_hint: segment.roughness_hint,
            gravel_hint: segment.gravel_hint,
            cutbank_hint: segment.cutbank_hint,
        })
        .collect::<Vec<_>>();

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
    if segment.role == GraphHydrologyRole::Trunk {
        return RiverReachType::Trunk;
    }

    let ratio = flow_ratio(segment.flow_accumulation, config);
    if ratio >= 0.75 {
        RiverReachType::Trunk
    } else if ratio >= 0.42 {
        RiverReachType::Lower
    } else if ratio >= 0.16 {
        RiverReachType::Middle
    } else if ratio >= 0.05 {
        RiverReachType::Upper
    } else {
        RiverReachType::Headwater
    }
}

fn morphology_for_segment(
    segment: &GraphRiverSegment,
    reach_type: RiverReachType,
    config: RiverPlanConfig,
) -> SegmentMorphology {
    let ratio = flow_ratio(segment.flow_accumulation, config);
    let q = segment.flow_accumulation.max(1.0);
    let width_coefficient = 0.75 + stable_unit(segment.edge.0 ^ 0x9e37_79b9) * 0.35;
    let depth_coefficient = 0.45 + stable_unit(segment.edge.0 ^ 0x85eb_ca6b) * 0.25;
    let hydraulic_width = width_coefficient * q.sqrt();
    let hydraulic_depth = depth_coefficient * q.powf(0.4);
    let reach_min = match reach_type {
        RiverReachType::Headwater => 0.0,
        RiverReachType::Upper | RiverReachType::LakeInlet => 0.04,
        RiverReachType::Middle => 0.12,
        RiverReachType::Lower | RiverReachType::LakeOutlet => 0.28,
        RiverReachType::Trunk => 0.45,
    };
    let scaled = ratio.max(reach_min).clamp(0.0, 1.0);
    let bed_width_blocks = hydraulic_width.clamp(2.0, 120.0);
    let bed_depth_blocks = hydraulic_depth.clamp(1.0, 32.0);

    SegmentMorphology {
        width_coefficient,
        depth_coefficient,
        velocity: (q / (hydraulic_width * hydraulic_depth).max(1.0)).max(0.0),
        broad_valley_width_blocks: (bed_width_blocks * (2.6 + scaled * 3.8)).clamp(8.0, 384.0),
        broad_valley_depth_blocks: (bed_depth_blocks * (1.4 + scaled * 2.4)).clamp(1.0, 72.0),
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

fn stable_unit(mut value: u64) -> f32 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    value = value.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    value ^= value >> 33;
    (value as f64 / u64::MAX as f64) as f32
}
