use std::path::PathBuf;

use crate::world::{
    ChunkCoord, ChunkData, CpuMesh, CreatedWorldManifest, TopdownChunkColumnCoord,
    TopdownChunkColumnPatch,
};

use super::request::JobRequest;

#[derive(Debug, Clone, PartialEq)]
pub enum JobResult {
    WorldCreated {
        root: PathBuf,
        manifest: CreatedWorldManifest,
    },
    ChunkLoaded { coord: ChunkCoord, chunk: ChunkData },
    ChunkGenerated { coord: ChunkCoord, chunk: ChunkData },
    ChunkMeshBuilt { coord: ChunkCoord, mesh: CpuMesh },
    MinimapChunkColumnBuilt {
        coord: TopdownChunkColumnCoord,
        patch: TopdownChunkColumnPatch,
    },
    JobFailed { request: JobRequest, error: JobError },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobError {
    Shutdown,
    WorkerDisconnected { worker_id: usize },
    ExecutionFailed { message: String },
}

impl JobResult {
    pub fn coord(&self) -> ChunkCoord {
        match self {
            Self::WorldCreated { .. } => {
                panic!("WorldCreated result does not map to a single chunk coordinate")
            }
            Self::ChunkLoaded { coord, .. }
            | Self::ChunkGenerated { coord, .. }
            | Self::ChunkMeshBuilt { coord, .. } => *coord,
            Self::MinimapChunkColumnBuilt { .. } => {
                panic!("MinimapChunkColumnBuilt result does not map to a single chunk coordinate")
            }
            Self::JobFailed { request, .. } => request.coord(),
        }
    }
}
