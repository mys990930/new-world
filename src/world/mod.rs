pub mod generation;
pub mod legacy;

#[allow(unused_imports)]
pub use generation::{
    ColumnSynthesisRequest, ColumnSynthesisSample, ContinuousFieldSample,
    DEFAULT_BASE_FIELD_SELF_WEIGHT, DEFAULT_BASE_FIELD_SMOOTHING_PASSES,
    DEFAULT_GRAPH_PADDING_REGIONS, DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS,
    GRAPH_GENERATION_STAGES, GraphBaseFieldConfig, GraphBaseFields, GraphDrainageNode,
    GraphDrainageNodeId, GraphDrainageNodeKind, GraphGenerationStage, GraphHydrologyGraph,
    GraphHydrologyRole, GraphInfluence, GraphRegionArea, GraphRegionCoord, GraphRiverSegment,
    GraphRiverSegmentId, GraphWorldGenerationConfig, VoronoiBlendSample, VoronoiCorner,
    VoronoiCornerId, VoronoiEdge, VoronoiEdgeId, VoronoiGraphConfig, VoronoiGraphPatch,
    VoronoiGraphPatchRequest, VoronoiSite, VoronoiSiteId, WatershedId, WorldPlanePoint,
    apply_base_graph_fields, clamp_unit_field, generate_voronoi_graph_patch,
    graph_generation_stages, graph_region_for_world_block, normalize_influences,
};

// Compatibility bridge while the current runtime still depends on the archived world module.
// New graph-first code should prefer `world::generation`; legacy exports remain available so the
// app, jobs, renderer input, storage, and existing tools can compile during the migration.
#[allow(unused_imports)]
pub use legacy::*;
