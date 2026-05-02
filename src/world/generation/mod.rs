pub mod field;
pub mod graph;
pub mod hydrology;
pub mod macro_map;
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
    DEFAULT_BASE_FIELD_SELF_WEIGHT, DEFAULT_BASE_FIELD_SMOOTHING_PASSES,
    DEFAULT_GRAPH_PADDING_REGIONS, DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS,
    GraphBaseFieldConfig, GraphBaseFields, GraphRegionArea, GraphRegionCoord, VoronoiCorner,
    VoronoiCornerId, VoronoiEdge, VoronoiEdgeId, VoronoiGraphConfig, VoronoiGraphPatch,
    VoronoiGraphPatchRequest, VoronoiSite, VoronoiSiteId, WorldPlanePoint, apply_base_graph_fields,
    generate_voronoi_graph_patch, graph_region_for_world_block,
};
#[allow(unused_imports)]
pub use hydrology::{
    DEFAULT_HEADWATER_ELEVATION, DEFAULT_RIVER_FLOW_THRESHOLD, GraphDrainageNode,
    GraphDrainageNodeId, GraphDrainageNodeKind, GraphHydrologyCorner, GraphHydrologyGraph,
    GraphHydrologyRole, GraphLocalMinimumResolution, GraphRiverSegment, GraphRiverSegmentId,
    HydrologyConfig, WatershedId, solve_hydrology,
};
#[allow(unused_imports)]
pub use macro_map::{
    DEFAULT_MACRO_COAST_WIDTH_BLOCKS, DEFAULT_MACRO_RIDGE_CANDIDATE_THRESHOLD,
    DEFAULT_MACRO_RIVER_CANDIDATE_THRESHOLD, GraphMacroMap, MacroContinentId, MacroCorner,
    MacroEdge, MacroEdgeGuide, MacroMapConfig, MacroOceanBasinId, MacroSite, MacroSurfaceKind,
    generate_macro_map,
};
#[allow(unused_imports)]
pub use pipeline::{
    ColumnSynthesisRequest, ColumnSynthesisSample, GRAPH_GENERATION_STAGES, GraphGenerationStage,
    GraphWorldGenerationConfig, graph_generation_stages,
};
