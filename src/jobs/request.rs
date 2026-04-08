use crate::world::{ChunkCoord, ChunkSnapshot, NeighborChunks, WorldMeta};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobRequest {
    GenerateChunk {
        coord: ChunkCoord,
        meta: WorldMeta,
    },
    BuildChunkMesh {
        center: ChunkSnapshot,
        neighbors: NeighborChunks,
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
