use crate::world::{ChunkCoord, ChunkData, CpuMesh};

use super::request::JobRequest;

#[derive(Debug, Clone, PartialEq)]
pub enum JobResult {
    ChunkLoaded { coord: ChunkCoord, chunk: ChunkData },
    ChunkGenerated { coord: ChunkCoord, chunk: ChunkData },
    ChunkMeshBuilt { coord: ChunkCoord, mesh: CpuMesh },
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
            Self::ChunkLoaded { coord, .. }
            | Self::ChunkGenerated { coord, .. }
            | Self::ChunkMeshBuilt { coord, .. } => *coord,
            Self::JobFailed { request, .. } => request.coord(),
        }
    }
}
