use crate::world::{build_chunk_mesh, generate_chunk};

use super::request::JobRequest;
use super::result::JobResult;

pub(crate) fn execute(request: JobRequest) -> JobResult {
    match request {
        JobRequest::GenerateChunk { coord, meta } => JobResult::ChunkGenerated {
            coord,
            chunk: generate_chunk(coord, &meta),
        },
        JobRequest::BuildChunkMesh { center, neighbors } => {
            let coord = center.coord();
            let mesh = build_chunk_mesh(&center, neighbors);
            JobResult::ChunkMeshBuilt { coord, mesh }
        }
    }
}
