pub mod field;
pub mod graph;
pub mod hydrology;
pub mod pipeline;

// Legacy generation remains available through `world::generation::*` until the
// graph-first pipeline replaces the current runtime generator entrypoints.
#[allow(unused_imports)]
pub use crate::world::legacy::generation::*;

#[allow(unused_imports)]
pub use field::{
    ContinuousFieldSample, GraphInfluence, VoronoiBlendSample, clamp_unit_field,
    normalize_influences,
};
#[allow(unused_imports)]
pub use graph::{
    DEFAULT_GRAPH_PADDING_REGIONS, DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS,
    GraphRegionArea, GraphRegionCoord, VoronoiCorner, VoronoiCornerId, VoronoiEdge, VoronoiEdgeId,
    VoronoiGraphConfig, VoronoiGraphPatch, VoronoiGraphPatchRequest, VoronoiSite, VoronoiSiteId,
    WorldPlanePoint, generate_voronoi_graph_patch, graph_region_for_world_block,
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
