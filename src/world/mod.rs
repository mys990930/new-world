pub mod atlas;
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

#[allow(unused_imports)]
pub use atlas::{
    ATLAS_CELL_SIZE_IN_CHUNKS, ATLAS_CELL_SIZE_IN_REGIONS, ATLAS_CELL_SIZE_M, AtlasArea,
    AtlasAreaError, AtlasCell, AtlasClimateTuning, AtlasColorRgb, AtlasContinentTuning,
    AtlasCoord, AtlasDebugError, AtlasDebugOptions, AtlasFieldMap, AtlasGrid,
    AtlasHydrologyTuning, AtlasNormalizationTuning, AtlasPreviewDebugTuning, AtlasResolvedCell,
    AtlasResolvedMap, AtlasResolverTuning, AtlasRidgeTuning, AtlasTerrainTuning, AtlasTuning,
    AtlasWeightTuning, BiomePreview, MoistureClass, OverlayClass, TerrainFormClass,
    ThermalClass, generate_atlas_fields, generate_atlas_fields_with_tuning, resolve_atlas,
    resolve_atlas_with_tuning, write_debug_images, write_debug_images_with_options,
    write_debug_images_with_options_and_tuning,
};
#[allow(unused_imports)]
pub use chunk::{BlockFace, BlockId, ChunkData, ChunkSnapshot, ChunkWriteError};
#[allow(unused_imports)]
pub use coord::{
    CHUNK_EDGE, CHUNK_EDGE_I32, CHUNK_VOLUME, ChunkCoord, LocalBlockCoord, WorldBlockCoord,
    chunk_local_to_world, is_local_in_bounds, world_to_chunk_local,
};
#[allow(unused_imports)]
pub use core::WorldCore;
#[allow(unused_imports)]
pub use edit::{EditError, EditResult, WorldEdit};
#[allow(unused_imports)]
pub use generation::{FLAT_WORLD_SURFACE_Y, generate_chunk};
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
