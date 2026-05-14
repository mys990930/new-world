use super::super::graph::{VoronoiCornerId, VoronoiEdgeId, WorldPlanePoint};

pub const DEFAULT_RIVER_FLOW_THRESHOLD: f32 = 100.0;
pub const DEFAULT_HEADWATER_ELEVATION: f32 = 0.10;
pub const DEFAULT_TRIBUTARY_SOURCE_THRESHOLD: f32 = 0.52;
pub const DEFAULT_TRIBUTARY_SOURCE_HYDRATION: f32 = 0.46;
pub const DEFAULT_TRIBUTARY_MAX_PATH_EDGES: u32 = 18;
pub(super) const DEFAULT_HEADWATER_SOURCE_HYDRATION: f32 = 0.34;
pub(super) const DEFAULT_HEADWATER_SOURCE_SCORE: f32 = 0.40;
pub(super) const LAKE_INLET_RIVER_THRESHOLD_CAP: f32 = 22.0;
pub const DEFAULT_HEADWATER_SOURCE_HYDRATION_FLOOR: f32 = 0.46;
pub const DEFAULT_LAKE_RIVER_FLOW_THRESHOLD_MULTIPLIER: f32 = 5.0;
pub const DEFAULT_LAKE_DISCHARGE_CAP_PER_AREA: f32 = 0.35;
pub const DEFAULT_LAKE_DISCHARGE_CAP_FLOOR: f32 = 4.0;
pub const DEFAULT_LAKE_DISCHARGE_CAP_CEILING: f32 = 32.0;
pub const DEFAULT_LAKE_DISCHARGE_RANGE_PER_AREA: f32 = 0.18;
pub const DEFAULT_LAKE_AREA_UNITS_PER_CHAIN: f32 = 24.0;
pub const DEFAULT_LAKE_MAX_INCOMING_CHAINS: usize = 3;
pub const DEFAULT_LAKE_MAX_OUTLETS_PER_COMPONENT: usize = 2;
pub const DEFAULT_LAKE_INLET_OUTLET_MIN_EDGE_HOPS: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HydrologyConfig {
    pub river_flow_threshold: f32,
    pub headwater_elevation: f32,
    pub tributary_source_threshold: f32,
    pub tributary_source_hydration: f32,
    pub tributary_max_path_edges: u32,
    pub lake_river_flow_threshold_multiplier: f32,
    pub lake_discharge_cap_per_area: f32,
    pub lake_discharge_cap_floor: f32,
    pub lake_discharge_cap_ceiling: f32,
    pub lake_discharge_range_per_area: f32,
    pub lake_area_units_per_chain: f32,
    pub lake_max_incoming_chains: usize,
    pub lake_max_outlets_per_component: usize,
    pub lake_inlet_outlet_min_edge_hops: u32,
}

impl Default for HydrologyConfig {
    fn default() -> Self {
        Self {
            river_flow_threshold: DEFAULT_RIVER_FLOW_THRESHOLD,
            headwater_elevation: DEFAULT_HEADWATER_ELEVATION,
            tributary_source_threshold: DEFAULT_TRIBUTARY_SOURCE_THRESHOLD,
            tributary_source_hydration: DEFAULT_TRIBUTARY_SOURCE_HYDRATION,
            tributary_max_path_edges: DEFAULT_TRIBUTARY_MAX_PATH_EDGES,
            lake_river_flow_threshold_multiplier: DEFAULT_LAKE_RIVER_FLOW_THRESHOLD_MULTIPLIER,
            lake_discharge_cap_per_area: DEFAULT_LAKE_DISCHARGE_CAP_PER_AREA,
            lake_discharge_cap_floor: DEFAULT_LAKE_DISCHARGE_CAP_FLOOR,
            lake_discharge_cap_ceiling: DEFAULT_LAKE_DISCHARGE_CAP_CEILING,
            lake_discharge_range_per_area: DEFAULT_LAKE_DISCHARGE_RANGE_PER_AREA,
            lake_area_units_per_chain: DEFAULT_LAKE_AREA_UNITS_PER_CHAIN,
            lake_max_incoming_chains: DEFAULT_LAKE_MAX_INCOMING_CHAINS,
            lake_max_outlets_per_component: DEFAULT_LAKE_MAX_OUTLETS_PER_COMPONENT,
            lake_inlet_outlet_min_edge_hops: DEFAULT_LAKE_INLET_OUTLET_MIN_EDGE_HOPS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WatershedId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphDrainageNodeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphRiverSegmentId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphDrainageNodeKind {
    Source,
    Confluence,
    LakeInlet,
    LakeOutlet,
    Lake,
    Sink,
    CoastOutlet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphHydrologyRole {
    None,
    Divide,
    Headwater,
    Tributary,
    Trunk,
    Floodplain,
    CoastOutlet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphLocalMinimumResolution {
    None,
    OceanOutlet,
    OutletCarve,
    Lake,
    Sink,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphHydrologyCorner {
    pub id: VoronoiCornerId,
    pub position: WorldPlanePoint,
    pub elevation: f32,
    pub downstream: Option<VoronoiCornerId>,
    pub downstream_edge: Option<VoronoiEdgeId>,
    pub watershed: WatershedId,
    pub flow_accumulation: f32,
    pub is_local_minimum: bool,
    pub resolution: GraphLocalMinimumResolution,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphDrainageNode {
    pub id: GraphDrainageNodeId,
    pub kind: GraphDrainageNodeKind,
    pub corner: VoronoiCornerId,
    pub position: WorldPlanePoint,
    pub watershed: WatershedId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphRiverSegment {
    pub id: GraphRiverSegmentId,
    pub edge: VoronoiEdgeId,
    pub from: GraphDrainageNodeId,
    pub to: GraphDrainageNodeId,
    pub watershed: WatershedId,
    pub role: GraphHydrologyRole,
    pub raw_flow_accumulation: f32,
    pub flow_accumulation: f32,
    pub downstream_progress: f32,
    pub local_slope: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GraphHydrologyTopologyStats {
    pub lake_inlet_count: usize,
    pub lake_outlet_count: usize,
    pub disconnected_lake_inlet_count: usize,
    pub disconnected_lake_outlet_count: usize,
    pub selected_lake_edge_segment_count: usize,
    pub invalid_lake_contact_count: usize,
    pub invalid_river_intersection_count: usize,
    pub ambiguous_shared_corner_count: usize,
    pub duplicate_trunk_pruned_count: usize,
    pub repeated_lake_contact_pruned_count: usize,
    pub disconnected_river_fragment_pruned_count: usize,
    pub unclassified_lake_connected_flow_count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct GraphHydrologyGraph {
    pub corners: Vec<GraphHydrologyCorner>,
    pub nodes: Vec<GraphDrainageNode>,
    pub segments: Vec<GraphRiverSegment>,
    pub topology_stats: GraphHydrologyTopologyStats,
}

impl GraphHydrologyGraph {
    pub fn is_empty(&self) -> bool {
        self.corners.is_empty() && self.nodes.is_empty() && self.segments.is_empty()
    }

    pub fn corner(&self, id: VoronoiCornerId) -> Option<&GraphHydrologyCorner> {
        self.corners.iter().find(|corner| corner.id == id)
    }

    pub fn segments_for_edge(
        &self,
        edge: VoronoiEdgeId,
    ) -> impl Iterator<Item = &GraphRiverSegment> {
        self.segments
            .iter()
            .filter(move |segment| segment.edge == edge)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct LakeContactTopology {
    pub(super) component_by_corner: Vec<Option<usize>>,
    pub(super) contact_component_by_land_corner: Vec<Option<usize>>,
    pub(super) inlet_vertices: Vec<bool>,
    pub(super) outlet_vertices: Vec<bool>,
    pub(super) inlet_land_vertices: Vec<bool>,
    pub(super) outlet_land_vertices: Vec<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct LakeTerminalPolicy {
    pub(super) area_units: u32,
    pub(super) component_root: usize,
    pub(super) max_incoming_chains: usize,
    pub(super) selection_threshold: f32,
    pub(super) discharge_floor: f32,
    pub(super) discharge_cap: f32,
}

pub(super) fn validate_hydrology_config(config: HydrologyConfig) {
    assert!(
        config.river_flow_threshold.is_finite() && config.river_flow_threshold > 0.0,
        "river_flow_threshold must be positive and finite"
    );
    assert!(
        config.headwater_elevation.is_finite(),
        "headwater_elevation must be finite"
    );
    assert!(
        config.tributary_source_threshold.is_finite() && config.tributary_source_threshold >= 0.0,
        "tributary_source_threshold must be finite and >= 0"
    );
    assert!(
        config.tributary_source_hydration.is_finite()
            && (0.0..=1.0).contains(&config.tributary_source_hydration),
        "tributary_source_hydration must be finite and within 0..=1"
    );
    assert!(
        config.tributary_max_path_edges > 0,
        "tributary_max_path_edges must be > 0"
    );
    assert!(
        config.lake_river_flow_threshold_multiplier.is_finite()
            && config.lake_river_flow_threshold_multiplier >= 1.0,
        "lake_river_flow_threshold_multiplier must be finite and >= 1.0"
    );
    assert!(
        config.lake_discharge_cap_per_area.is_finite() && config.lake_discharge_cap_per_area > 0.0,
        "lake_discharge_cap_per_area must be positive and finite"
    );
    assert!(
        config.lake_discharge_cap_floor.is_finite() && config.lake_discharge_cap_floor > 0.0,
        "lake_discharge_cap_floor must be positive and finite"
    );
    assert!(
        config.lake_discharge_cap_ceiling.is_finite()
            && config.lake_discharge_cap_ceiling >= config.lake_discharge_cap_floor,
        "lake_discharge_cap_ceiling must be finite and >= lake_discharge_cap_floor"
    );
    assert!(
        config.lake_discharge_range_per_area.is_finite()
            && config.lake_discharge_range_per_area >= 0.0,
        "lake_discharge_range_per_area must be finite and >= 0"
    );
    assert!(
        config.lake_area_units_per_chain.is_finite() && config.lake_area_units_per_chain > 0.0,
        "lake_area_units_per_chain must be positive and finite"
    );
    assert!(
        config.lake_max_incoming_chains > 0,
        "lake_max_incoming_chains must be > 0"
    );
    assert!(
        config.lake_max_outlets_per_component <= 2,
        "lake_max_outlets_per_component must be <= 2 for launch topology"
    );
    assert!(
        config.lake_inlet_outlet_min_edge_hops > 0,
        "lake_inlet_outlet_min_edge_hops must be > 0"
    );
}

pub(super) fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
