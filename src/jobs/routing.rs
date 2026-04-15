use crate::world::{
    build_chunk_mesh, create_world_to_directory, generate_chunk, load_created_world_chunk,
};

use super::request::JobRequest;
use super::result::{JobError, JobResult};

pub(crate) fn execute(request: JobRequest) -> JobResult {
    match request {
        JobRequest::CreateWorld {
            root,
            config,
            registry,
        } => match create_world_to_directory(root.as_path(), config, registry.as_ref()) {
            Ok(manifest) => JobResult::WorldCreated { root, manifest },
            Err(error) => JobResult::JobFailed {
                request: JobRequest::CreateWorld {
                    root,
                    config,
                    registry,
                },
                error: JobError::ExecutionFailed {
                    message: error.to_string(),
                },
            },
        },
        JobRequest::LoadChunk { root, coord } => match load_created_world_chunk(root.as_path(), coord) {
            Ok(chunk) => JobResult::ChunkLoaded { coord, chunk },
            Err(error) => JobResult::JobFailed {
                request: JobRequest::LoadChunk { root, coord },
                error: JobError::ExecutionFailed {
                    message: error.to_string(),
                },
            },
        },
        JobRequest::GenerateChunk {
            coord,
            meta,
            registry,
        } => JobResult::ChunkGenerated {
            coord,
            chunk: generate_chunk(coord, &meta, registry.as_ref()),
        },
        JobRequest::BuildChunkMesh {
            center,
            neighbors,
            registry,
        } => {
            let coord = center.coord();
            let mesh = build_chunk_mesh(&center, neighbors, registry.as_ref());
            JobResult::ChunkMeshBuilt { coord, mesh }
        }
    }
}
