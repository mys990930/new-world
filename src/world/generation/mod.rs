pub mod boundary;
pub mod field;
pub mod graph;
pub mod heightfield;
pub mod hydrology;
pub mod macro_field;
pub mod macro_map;
pub mod pipeline;

// Legacy generation remains available through `world::generation::*` until the
// graph-first pipeline replaces the current runtime generator entrypoints.
#[allow(unused_imports)]
pub use crate::world::legacy::generation::*;

#[allow(unused_imports)]
pub use boundary::{
    BoundaryAnchors, BoundaryCache, BoundaryConfig, BoundaryGuard, BoundaryProfile, BoundaryStats,
    DEFAULT_BOUNDARY_GUARD_MARGIN_BLOCKS, DEFAULT_BOUNDARY_MAX_EDGE_FRACTION,
    DEFAULT_BOUNDARY_MAX_SITE_SPAN_FRACTION, DEFAULT_BOUNDARY_MAX_VISIBLE_AMPLITUDE_BLOCKS,
    DEFAULT_BOUNDARY_MIN_VISIBLE_AMPLITUDE_BLOCKS, DEFAULT_BOUNDARY_SUBDIVISION_LEVELS,
    NoisyBoundaryCurve, generate_noisy_boundaries,
};
#[allow(unused_imports)]
pub use field::{
    ContinuousFieldSample, GraphInfluence, VoronoiBlendSample, clamp_unit_field,
    normalize_influences,
};
#[allow(unused_imports)]
pub use graph::{
    DEFAULT_BASE_FIELD_SELF_WEIGHT, DEFAULT_BASE_FIELD_SMOOTHING_PASSES,
    DEFAULT_GRAPH_PADDING_REGIONS, DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS,
    GraphBaseFieldConfig, GraphBaseFields, GraphRegionArea, GraphRegionCoord,
    GraphSiteSpacingStats, MIN_NEAREST_SITE_SPACING_FRACTION, VoronoiCorner, VoronoiCornerId,
    VoronoiEdge, VoronoiEdgeId, VoronoiGraphConfig, VoronoiGraphPatch, VoronoiGraphPatchRequest,
    VoronoiSite, VoronoiSiteId, WorldPlanePoint, apply_base_graph_fields,
    generate_voronoi_graph_patch, graph_region_for_world_block, graph_site_spacing_stats,
};
#[allow(unused_imports)]
pub use heightfield::{
    DEFAULT_HEIGHTFIELD_LAKE_BED_BLOCKS, DEFAULT_HEIGHTFIELD_MAX_BLOCKS,
    DEFAULT_HEIGHTFIELD_MIN_BLOCKS, DEFAULT_HEIGHTFIELD_NORMALIZED_MAX,
    DEFAULT_HEIGHTFIELD_NORMALIZED_MIN, DEFAULT_HEIGHTFIELD_OCEAN_BED_BLOCKS,
    DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD, DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
    HeightfieldColumn, HeightfieldConfig, HeightfieldTerrainKind, HeightfieldTile,
    HeightfieldTileStats, generate_heightfield_tile, heightfield_column_from_sample,
};
#[allow(unused_imports)]
pub use hydrology::{
    DEFAULT_HEADWATER_ELEVATION, DEFAULT_LAKE_DISCHARGE_CAP_FLOOR,
    DEFAULT_LAKE_DISCHARGE_CAP_PER_AREA, DEFAULT_LAKE_RIVER_FLOW_THRESHOLD_MULTIPLIER,
    DEFAULT_RIVER_FLOW_THRESHOLD, GraphDrainageNode, GraphDrainageNodeId, GraphDrainageNodeKind,
    GraphHydrologyCorner, GraphHydrologyGraph, GraphHydrologyRole, GraphHydrologyTopologyStats,
    GraphLocalMinimumResolution, GraphRiverSegment, GraphRiverSegmentId, HydrologyConfig,
    WatershedId, solve_hydrology,
};
#[allow(unused_imports)]
pub use macro_field::{
    DEFAULT_MACRO_FIELD_BOUNDARY_BLEND_RADIUS_BLOCKS, DEFAULT_MACRO_FIELD_COAST_FLATTEN_STRENGTH,
    DEFAULT_MACRO_FIELD_COAST_RADIUS_BLOCKS, DEFAULT_MACRO_FIELD_LAKE_FLATTEN_STRENGTH,
    DEFAULT_MACRO_FIELD_RIDGE_HEIGHT_SCALE, DEFAULT_MACRO_FIELD_RIDGE_RADIUS_BLOCKS,
    DEFAULT_MACRO_FIELD_RIVER_CARVE_SCALE, DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS,
    DEFAULT_MACRO_FIELD_SAMPLE_SPACING_BLOCKS, MacroFieldRasterContext, MacroFieldSample,
    MacroFieldTile, MacroFieldTileConfig, MacroFieldTileStats, generate_macro_field_tile,
    sample_macro_field_point,
};
#[allow(unused_imports)]
pub use macro_map::{
    DEFAULT_MACRO_COAST_WIDTH_BLOCKS, DEFAULT_MACRO_RIDGE_CANDIDATE_THRESHOLD,
    DEFAULT_MACRO_RIVER_CANDIDATE_THRESHOLD, GraphMacroMap, MacroContinentId, MacroCorner,
    MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroMapConfig, MacroOceanBasinId, MacroSite,
    MacroSurfaceKind, generate_macro_map,
};
#[allow(unused_imports)]
pub use pipeline::{
    ColumnSynthesisRequest, ColumnSynthesisSample, GRAPH_GENERATION_STAGES, GraphGenerationStage,
    GraphWorldGenerationConfig, graph_generation_stages,
};
