use std::path::PathBuf;
use std::sync::Arc;

use crate::world::{
    BlockRegistry, ChunkCoord, ChunkSnapshot, CreateWorldConfig, NeighborChunks,
    TopdownChunkColumnCoord, WorldMeta,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobRequest {
    CreateWorld {
        root: PathBuf,
        config: CreateWorldConfig,
        registry: Arc<BlockRegistry>,
    },
    LoadChunk {
        root: PathBuf,
        coord: ChunkCoord,
    },
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
    BuildMinimapChunkColumn {
        coord: TopdownChunkColumnCoord,
        chunks: Vec<ChunkSnapshot>,
        registry: Arc<BlockRegistry>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum JobCoalesceKey {
    CreateWorld(PathBuf),
    LoadChunk(ChunkCoord),
    GenerateChunk(ChunkCoord),
    BuildChunkMesh(ChunkCoord),
    BuildMinimapChunkColumn(TopdownChunkColumnCoord),
}

impl JobRequest {
    pub fn coord(&self) -> ChunkCoord {
        match self {
            Self::CreateWorld { .. } => {
                panic!("CreateWorld request does not map to a single chunk coordinate")
            }
            Self::LoadChunk { coord, .. } | Self::GenerateChunk { coord, .. } => *coord,
            Self::BuildChunkMesh { center, .. } => center.coord(),
            Self::BuildMinimapChunkColumn { .. } => {
                panic!("BuildMinimapChunkColumn request does not map to a single chunk coordinate")
            }
        }
    }

    pub(crate) fn coalesce_key(&self) -> JobCoalesceKey {
        match self {
            Self::CreateWorld { root, .. } => JobCoalesceKey::CreateWorld(root.clone()),
            Self::LoadChunk { coord, .. } => JobCoalesceKey::LoadChunk(*coord),
            Self::GenerateChunk { coord, .. } => JobCoalesceKey::GenerateChunk(*coord),
            Self::BuildChunkMesh { center, .. } => JobCoalesceKey::BuildChunkMesh(center.coord()),
            Self::BuildMinimapChunkColumn { coord, .. } => {
                JobCoalesceKey::BuildMinimapChunkColumn(*coord)
            }
        }
    }
}
