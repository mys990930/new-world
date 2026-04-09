use crate::world::{build_chunk_mesh, generate_chunk};

use super::request::JobRequest;
use super::result::JobResult;

pub(crate) fn execute(request: JobRequest) -> JobResult {
    match request {
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
