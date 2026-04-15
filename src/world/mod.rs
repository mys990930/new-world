pub mod atlas;
mod created;
mod chunk;
mod coord;
mod core;
mod edit;
mod generation;
mod meshing;
mod meta;
mod query;
mod registry;
mod storage;
mod topdown;

#[allow(unused_imports)]
pub use atlas::{
    ATLAS_CELL_SIZE_IN_CHUNKS, ATLAS_CELL_SIZE_IN_REGIONS, ATLAS_CELL_SIZE_M, AtlasArea,
    AtlasAreaError, AtlasCell, AtlasClimateTuning, AtlasColorRgb, AtlasContinentTuning,
    AtlasCoord, AtlasDebugError, AtlasDebugOptions, AtlasFieldMap, AtlasGrid,
    AtlasHydrologyTuning, AtlasNormalizationTuning, AtlasPreviewDebugTuning, AtlasResolvedCell,
    AtlasResolvedMap, AtlasResolverTuning, AtlasRidgeTuning, AtlasStructureMap,
    AtlasStructureRegion, AtlasStructureRegionCoord, AtlasTerrainTuning, AtlasTuning,
    AtlasWeightTuning, BiomePreview, DrainageGraph, DrainageNode, DrainageNodeKind,
    MESO_GUIDE_CELL_SIZE_IN_CHUNKS, MESO_GUIDE_CELLS_PER_ATLAS_CELL, MESO_REGION_EDGE_CELLS,
    MoistureClass, MesoGuideCell, MesoGuideMap, MesoGuideSample, MesoRegion, MesoRegionCoord,
    MountainChainGraph, MountainChainId, MountainChainScale, MountainSpineSegment, OverlayClass,
    RiverPathId, RiverPathKind, RiverPathSegment, TerrainFormClass, ThermalClass,
    ATLAS_STRUCTURE_REGION_EDGE_CELLS, ATLAS_STRUCTURE_REGION_PADDING_CELLS,
    atlas_structure_region_coord_for_atlas, atlas_structure_regions_covering_area,
    generate_atlas_fields, generate_atlas_fields_with_tuning, generate_atlas_structure,
    generate_atlas_structure_with_tuning, generate_meso_guides, meso_region_coord_for_atlas,
    meso_regions_covering_area, resolve_atlas, resolve_atlas_with_tuning, sample_meso_guides,
    write_debug_images, write_debug_images_with_options, write_debug_images_with_options_and_tuning,
};
#[allow(unused_imports)]
pub use created::{
    CREATED_WORLD_MANIFEST_FILE, CreateWorldConfig, CreatedWorldStackSummary, CreatedWorldError,
    CreatedWorldManifest, CreatedWorldSource, create_world_to_directory, created_world_chunk_path,
    created_world_manifest_path, detect_latest_created_world_root, load_created_world_chunk,
    read_created_world_manifest, save_created_world_chunk, summarize_created_world_stack,
    write_created_world_manifest,
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
pub use edit::{EditError, EditResult, WorldEdit};
#[allow(unused_imports)]
pub use generation::{FLAT_WORLD_SURFACE_Y, SEA_LEVEL_Y, WORLD_FLOOR_Y, generate_chunk};
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
pub use topdown::{
    TopdownCell, TopdownChunkColumnCoord, TopdownChunkColumnPatch, TopdownColumnScan,
    TopdownEdge, TopdownSurfaceRange, color_topdown_cell, darken_topdown_color,
    sample_single_topdown_column, sample_topdown_chunk_column, sample_topdown_columns,
    topdown_edge_strength_for_cell, topdown_outline_strength, topdown_surface_range,
};
