pub mod atlas;
mod calendar;
mod chunk;
mod coord;
mod core;
mod created;
mod edit;
mod generation;
mod meshing;
mod meta;
mod query;
mod registry;
mod storage;
mod surface;
mod topdown;

#[allow(unused_imports)]
pub use atlas::{
    ATLAS_CELL_SIZE_IN_CHUNKS, ATLAS_CELL_SIZE_IN_REGIONS, ATLAS_CELL_SIZE_M,
    ATLAS_STRUCTURE_REGION_EDGE_CELLS, ATLAS_STRUCTURE_REGION_PADDING_CELLS, AtlasArea,
    AtlasAreaError, AtlasCell, AtlasClimateTuning, AtlasColorRgb, AtlasContinentTuning, AtlasCoord,
    AtlasDebugError, AtlasDebugOptions, AtlasFieldMap, AtlasGrid, AtlasHydrologyTuning,
    AtlasNormalizationTuning, AtlasPreviewDebugTuning, AtlasResolvedCell, AtlasResolvedMap,
    AtlasResolverTuning, AtlasRidgeTuning, AtlasStructureMap, AtlasStructureRegion,
    AtlasStructureRegionCoord, AtlasTerrainTuning, AtlasTuning, AtlasWeightTuning, BiomeFamily,
    BiomePreview, ClimateRegime, CoastalContext, DrainageBasinId, DrainageGraph, DrainageNode,
    DrainageNodeKind, ElevationBand, HydrologyContext, MESO_GUIDE_CELL_SIZE_IN_CHUNKS,
    MESO_GUIDE_CELLS_PER_ATLAS_CELL, MESO_REGION_EDGE_CELLS, MesoCatalogEntry, MesoCatalogStatus,
    MesoFeatureDef, MesoGuideCell, MesoGuideMap, MesoGuideSample, MesoRegion, MesoRegionCoord,
    MoistureBand, MoistureClass, MountainChainGraph, MountainChainId, MountainChainScale,
    MountainSpineSegment, OverlayClass, RAW_CLASSIFICATION_DIMENSIONS,
    RESOLVED_CLASSIFICATION_DIMENSIONS, RawClassificationDimension, RegionArchetype,
    RegionArchetypeDef, RegionCatalogEntry, RegionCatalogStatus, RegionClassCell,
    RegionClassInfluence, RegionClassInfluenceSet, RegionClassMap, RegionClassSample, ReliefClass,
    RiverPathId, RiverPathKind, RiverPathSegment, TemperatureBand, TerrainFormClass,
    TerrainFormFamily, ThermalClass, atlas_structure_region_coord_for_atlas,
    atlas_structure_regions_covering_area, generate_atlas_fields,
    generate_atlas_fields_with_tuning, generate_atlas_structure,
    generate_atlas_structure_with_tuning, generate_meso_guides, meso_catalog_entries,
    meso_feature_def, meso_feature_defs, meso_region_coord_for_atlas, meso_regions_covering_area,
    region_archetype_def, region_archetype_defs, region_catalog_entries, resolve_atlas,
    resolve_atlas_with_tuning, resolve_region_classes, sample_meso_guides,
    sample_region_class_influences, sample_region_classes, write_debug_images,
    write_debug_images_with_options, write_debug_images_with_options_and_tuning,
};
#[allow(unused_imports)]
pub use calendar::{
    AtlasClimateRuntimeState, AtlasClimateRuntimeUpdate, CalendarAdvance, CalendarApplyResult,
    DeferredSeasonPatch, DeferredSeasonPatchKind, DeferredSeasonPatchTarget, LocalWeatherKind,
    LocalWeatherState, LocalWeatherUpdate, WorldCalendar,
};
#[allow(unused_imports)]
pub use chunk::{BlockFace, BlockId, ChunkData, ChunkSnapshot, ChunkWriteError};
#[allow(unused_imports)]
pub use coord::{
    BLOCK_SIZE_M, BLOCKS_PER_METER, CHUNK_EDGE, CHUNK_EDGE_I32, CHUNK_EDGE_M, CHUNK_VOLUME,
    ChunkCoord, LocalBlockCoord, WorldBlockCoord, chunk_local_to_world, is_local_in_bounds,
    world_to_chunk_local,
};
#[allow(unused_imports)]
pub use core::WorldCore;
#[allow(unused_imports)]
pub use created::{
    CREATED_WORLD_MANIFEST_FILE, CreateWorldConfig, CreateWorldProgress, CreatedWorldError,
    CreatedWorldManifest, CreatedWorldSource, CreatedWorldStackSummary, create_world_to_directory,
    create_world_to_directory_with_progress, created_world_chunk_path, created_world_manifest_path,
    detect_latest_created_world_root, load_created_world_chunk, read_created_world_manifest,
    save_created_world_chunk, summarize_created_world_stack,
    summarize_created_world_stack_from_voxelization_plan, write_created_world_manifest,
};
#[allow(unused_imports)]
pub use edit::{EditError, EditResult, WorldEdit};
#[allow(unused_imports)]
pub use generation::{
    BaseHeightfieldPrototype, ChunkCorridorWindow, ChunkGenerationInputCache,
    ChunkGenerationInputs, ChunkGenerationScaffold, ChunkRealizationFieldPatch,
    FLAT_WORLD_SURFACE_Y, GENERATOR_LABEL, GenerationScaffoldStage, HydrologyColumn, HydrologyMode,
    HydrologySolve, MesoAppliedColumn, MesoAppliedPrototype, PrototypeColumn,
    REALIZATION_NODE_BLOCK_SPAN, REALIZATION_NODE_CHUNK_SPAN, RealizationFieldNode,
    RealizationMaterialSupport, RealizationSample, RiverCorridorConstraint, SEA_LEVEL_Y,
    SmoothedColumn, SmoothedPrototype, VoxelizationColumnPlan, VoxelizationPlan, WORLD_FLOOR_Y,
    build_chunk_base_heightfield_prototype, build_chunk_corridor_window,
    build_chunk_generation_scaffold, build_chunk_generation_voxelization_plan,
    build_chunk_hydrology_solve, build_chunk_meso_applied_prototype,
    build_chunk_meso_applied_prototype_for_feature, build_chunk_realization_field_patch,
    build_chunk_smoothed_prototype, build_chunk_voxelization_plan, chunk_generation_input_area,
    default_voxelization_plan, empty_base_heightfield_prototype, empty_chunk_corridor_window,
    empty_chunk_realization_field_patch, empty_hydrology_solve, empty_meso_applied_prototype,
    empty_smoothed_prototype, generate_chunk, generate_chunk_from_generation_inputs,
    generate_chunk_from_voxelization_plan, prepare_chunk_generation_inputs,
    sample_chunk_realization_field, voxelize_chunk, voxelize_chunk_at_coord,
};
#[allow(unused_imports)]
pub use generation::{
    ChunkGenerationProbe, ChunkSurfaceLodGrid, ChunkSurfaceLodSample, ColumnAtlasSample,
    ColumnGenerationProbe, TerrainProfile, TerrainProfileCounts, probe_chunk, probe_column,
    sample_chunk_surface_lod,
};
#[allow(unused_imports)]
pub use meshing::{CpuMesh, MeshVertex, RenderBounds, build_chunk_mesh};
#[allow(unused_imports)]
pub use meta::WorldMeta;
#[allow(unused_imports)]
pub use query::{NeighborChunks, Ray3, RaycastHit};
#[allow(unused_imports)]
pub use registry::{
    BlockDef, BlockMaterialKind, BlockRegistry, BlockRegistryError, BlockRenderKind,
    FaceTextureSet, TextureTileDef, TextureTileId, TextureTileSource,
};
#[allow(unused_imports)]
pub use storage::{StorageError, load_chunk, save_chunk};
#[allow(unused_imports)]
pub use surface::{
    ChunkSurfacePlan, CoverOverrideRule, CoverPhase, MaterialPolicyDef, MaterialPolicyId,
    SeasonalBiomeStateDef, SeasonalBiomeStateId, SeasonalPhase, SurfaceColumnPlan,
    SurfaceRuntimeContext, cover_override_rule, default_cover_override_rules,
    default_material_policies, default_seasonal_biome_states, empty_chunk_surface_plan,
    material_policy_def, resolve_chunk_surface_plan, resolve_chunk_surface_plan_with_runtime,
    resolve_material_policy_for_archetype, seasonal_biome_state_def,
};
#[allow(unused_imports)]
pub use topdown::{
    TopdownCell, TopdownChunkColumnCoord, TopdownChunkColumnPatch, TopdownColumnScan, TopdownEdge,
    TopdownSurfaceRange, color_topdown_cell, darken_topdown_color, sample_single_topdown_column,
    sample_topdown_chunk_column, sample_topdown_columns, topdown_edge_strength_for_cell,
    topdown_outline_strength, topdown_surface_range,
};
