pub mod generation;
pub mod legacy;

#[allow(unused_imports)]
pub use generation::{
    ColumnSynthesisRequest, ColumnSynthesisSample, ContinuousFieldSample, GRAPH_GENERATION_STAGES,
    GraphDrainageNode, GraphDrainageNodeId, GraphDrainageNodeKind, GraphGenerationStage,
    GraphHydrologyGraph, GraphHydrologyRole, GraphInfluence, GraphRegionArea, GraphRegionCoord,
    GraphRiverSegment, GraphRiverSegmentId, GraphWorldGenerationConfig, VoronoiBlendSample,
    VoronoiCorner, VoronoiCornerId, VoronoiEdge, VoronoiEdgeId, VoronoiGraphPatch, VoronoiSite,
    VoronoiSiteId, WatershedId, WorldPlanePoint, clamp_unit_field, graph_generation_stages,
    graph_region_for_world_block, normalize_influences,
};

// Compatibility bridge while the current runtime still depends on the archived world module.
// New graph-first code should prefer `world::generation`; legacy exports remain available so the
// app, jobs, renderer input, storage, and existing tools can compile during the migration.
#[allow(unused_imports)]
pub use legacy::*;
