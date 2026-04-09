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
    BlockDef, BlockRegistry, BlockRegistryError, BlockRenderKind, FaceTextureSet, TextureTileDef,
    TextureTileId, TextureTileSource,
};
#[allow(unused_imports)]
pub use storage::{StorageError, load_chunk, save_chunk};
