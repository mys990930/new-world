pub mod field;
pub mod graph;
pub mod hydrology;
pub mod legacy;
pub mod pipeline;

#[allow(unused_imports)]
pub use field::{
    ContinuousFieldSample, GraphInfluence, VoronoiBlendSample, clamp_unit_field,
    normalize_influences,
};
#[allow(unused_imports)]
pub use graph::{
    DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS, GraphRegionArea,
    GraphRegionCoord, VoronoiCorner, VoronoiCornerId, VoronoiEdge, VoronoiEdgeId,
    VoronoiGraphPatch, VoronoiSite, VoronoiSiteId, WorldPlanePoint, graph_region_for_world_block,
};
#[allow(unused_imports)]
pub use hydrology::{
    GraphDrainageNode, GraphDrainageNodeId, GraphDrainageNodeKind, GraphHydrologyGraph,
    GraphHydrologyRole, GraphRiverSegment, GraphRiverSegmentId, WatershedId,
};
#[allow(unused_imports)]
pub use pipeline::{
    ColumnSynthesisRequest, ColumnSynthesisSample, GRAPH_GENERATION_STAGES, GraphGenerationStage,
    GraphWorldGenerationConfig, graph_generation_stages,
};

// Compatibility bridge while the current runtime still depends on the archived world module.
// New graph-first code should prefer the modules above; legacy exports remain available so the
// app, jobs, renderer input, storage, and existing tools can compile during the migration.
#[allow(unused_imports)]
pub use legacy::*;
