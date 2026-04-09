use std::sync::Arc;

use crate::world::{BlockRegistry, ChunkCoord, ChunkSnapshot, NeighborChunks, WorldMeta};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobRequest {
    GenerateChunk {
        coord: ChunkCoord,
        meta: WorldMeta,
        registry: Arc<BlockRegistry>,
    },
    BuildChunkMesh {
        center: ChunkSnapshot,
        neighbors: NeighborChunks,
        registry: Arc<BlockRegistry>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum JobCoalesceKey {
    GenerateChunk(ChunkCoord),
    BuildChunkMesh(ChunkCoord),
}

impl JobRequest {
    pub fn coord(&self) -> ChunkCoord {
        match self {
            Self::GenerateChunk { coord, .. } => *coord,
            Self::BuildChunkMesh { center, .. } => center.coord(),
        }
    }

    pub(crate) fn coalesce_key(&self) -> JobCoalesceKey {
        match self {
            Self::GenerateChunk { coord, .. } => JobCoalesceKey::GenerateChunk(*coord),
            Self::BuildChunkMesh { center, .. } => JobCoalesceKey::BuildChunkMesh(center.coord()),
        }
    }
}
