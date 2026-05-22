use std::path::PathBuf;

use crate::world::{
    AtlasArea, ChunkCoord, ChunkData, CpuMesh, CreatedWorldManifest, RegionClassMap,
    TopdownChunkColumnCoord, TopdownChunkColumnPatch,
};

use super::request::JobRequest;

#[derive(Debug, Clone, PartialEq)]
pub enum JobResult {
    CreateWorldProgress {
        root: PathBuf,
        completed_chunks: u32,
        total_chunks: u32,
    },
    WorldCreated {
        root: PathBuf,
        manifest: CreatedWorldManifest,
    },
    ChunkLoaded {
        coord: ChunkCoord,
        chunk: ChunkData,
    },
    ChunkGenerated {
        coord: ChunkCoord,
        chunk: ChunkData,
    },
    ChunkUnloaded {
        coord: ChunkCoord,
    },
    ChunkMeshBuilt {
        coord: ChunkCoord,
        mesh: CpuMesh,
    },
    MinimapChunkColumnBuilt {
        coord: TopdownChunkColumnCoord,
        patch: TopdownChunkColumnPatch,
    },
    RegionClassResolved {
        area: AtlasArea,
        classes: RegionClassMap,
    },
    JobFailed {
        request: JobRequest,
        error: JobError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobError {
    Shutdown,
    WorkerDisconnected { worker_id: usize },
    ExecutionFailed { message: String },
}

impl JobResult {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::CreateWorldProgress { .. } => "CreateWorldProgress",
            Self::WorldCreated { .. } => "WorldCreated",
            Self::ChunkLoaded { .. } => "ChunkLoaded",
            Self::ChunkGenerated { .. } => "ChunkGenerated",
            Self::ChunkUnloaded { .. } => "ChunkUnloaded",
            Self::ChunkMeshBuilt { .. } => "ChunkMeshBuilt",
            Self::MinimapChunkColumnBuilt { .. } => "MinimapChunkColumnBuilt",
            Self::RegionClassResolved { .. } => "RegionClassResolved",
            Self::JobFailed { .. } => "JobFailed",
        }
    }

    pub fn diagnostic_label(&self) -> String {
        match self {
            Self::CreateWorldProgress {
                root,
                completed_chunks,
                total_chunks,
            } => format!(
                "CreateWorldProgress(root={} chunks={}/{})",
                root.display(),
                completed_chunks,
                total_chunks
            ),
            Self::WorldCreated { root, manifest } => {
                format!(
                    "WorldCreated(root={} stacks={})",
                    root.display(),
                    manifest.stacks.len()
                )
            }
            Self::ChunkLoaded { coord, .. } => {
                format!("ChunkLoaded(pos=({}, {}, {}))", coord.0, coord.1, coord.2)
            }
            Self::ChunkGenerated { coord, .. } => format!(
                "ChunkGenerated(pos=({}, {}, {}))",
                coord.0, coord.1, coord.2
            ),
            Self::ChunkUnloaded { coord } => {
                format!("ChunkUnloaded(pos=({}, {}, {}))", coord.0, coord.1, coord.2)
            }
            Self::ChunkMeshBuilt { coord, mesh } => format!(
                "ChunkMeshBuilt(pos=({}, {}, {}) triangles={})",
                coord.0,
                coord.1,
                coord.2,
                mesh.triangle_count()
            ),
            Self::MinimapChunkColumnBuilt { coord, .. } => format!(
                "MinimapChunkColumnBuilt(column=({}, {}))",
                coord.chunk_x, coord.chunk_z
            ),
            Self::RegionClassResolved { area, .. } => format!(
                "RegionClassResolved(origin=({}, {}) size={}x{})",
                area.origin().x,
                area.origin().z,
                area.width(),
                area.height()
            ),
            Self::JobFailed { request, error } => {
                format!(
                    "JobFailed(request={} error={:?})",
                    request.diagnostic_label(),
                    error
                )
            }
        }
    }

    pub fn coord(&self) -> ChunkCoord {
        match self {
            Self::CreateWorldProgress { .. } => {
                panic!("CreateWorldProgress result does not map to a single chunk coordinate")
            }
            Self::WorldCreated { .. } => {
                panic!("WorldCreated result does not map to a single chunk coordinate")
            }
            Self::ChunkLoaded { coord, .. }
            | Self::ChunkGenerated { coord, .. }
            | Self::ChunkUnloaded { coord }
            | Self::ChunkMeshBuilt { coord, .. } => *coord,
            Self::MinimapChunkColumnBuilt { .. } => {
                panic!("MinimapChunkColumnBuilt result does not map to a single chunk coordinate")
            }
            Self::RegionClassResolved { .. } => {
                panic!("RegionClassResolved result does not map to a single chunk coordinate")
            }
            Self::JobFailed { request, .. } => request.coord(),
        }
    }
}
