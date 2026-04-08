use crate::jobs::{JobRequest, JobResult};
use crate::world::{ChunkCoord, WorldBlockCoord, WorldCore};

use super::{ChunkStates, EcsRuntime};

impl EcsRuntime {
    pub fn plan_chunk_job_requests(&mut self, world: &WorldCore) -> Vec<JobRequest> {
        let target_chunk = self.focused_player_chunk();
        let mut chunk_states = self.world_mut().resource_mut::<ChunkStates>();
        chunk_states.focus_single(target_chunk);

        let mut requests = Vec::new();

        if !chunk_states.loaded.contains(&target_chunk)
            && !chunk_states.generation_requested.contains(&target_chunk)
        {
            requests.push(JobRequest::GenerateChunk {
                coord: target_chunk,
                meta: *world.meta(),
            });
            chunk_states.generation_requested.insert(target_chunk);
            return requests;
        }

        if chunk_states.loaded.contains(&target_chunk)
            && !chunk_states.render_ready.contains(&target_chunk)
            && !chunk_states.mesh_requested.contains(&target_chunk)
        {
            if let Some(center) = world.snapshot_chunk(target_chunk) {
                requests.push(JobRequest::BuildChunkMesh {
                    center,
                    neighbors: world.query_neighbors(target_chunk),
                });
                chunk_states.mesh_requested.insert(target_chunk);
            }
        }

        requests
    }

    pub fn apply_job_result(&mut self, result: &JobResult) {
        let mut chunk_states = self.world_mut().resource_mut::<ChunkStates>();

        match result {
            JobResult::ChunkGenerated { coord, .. } => {
                chunk_states.generation_requested.remove(coord);
                chunk_states.loaded.insert(*coord);
            }
            JobResult::ChunkMeshBuilt { coord, .. } => {
                chunk_states.mesh_requested.remove(coord);
                chunk_states.render_ready.insert(*coord);
            }
            JobResult::JobFailed { request, .. } => match request {
                JobRequest::GenerateChunk { coord, .. } => {
                    chunk_states.generation_requested.remove(coord);
                }
                JobRequest::BuildChunkMesh { center, .. } => {
                    chunk_states.mesh_requested.remove(&center.coord());
                }
            },
        }
    }

    pub fn visible_chunks(&self) -> Vec<ChunkCoord> {
        self.world().resource::<ChunkStates>().visible_chunks()
    }

    fn focused_player_chunk(&self) -> ChunkCoord {
        let world = self
            .local_player_transform()
            .map(|transform| transform.translation)
            .unwrap_or([8.0, 1.5, 8.0]);
        let block = WorldBlockCoord(
            world[0].floor() as i32,
            world[1].floor() as i32,
            world[2].floor() as i32,
        );
        crate::world::world_to_chunk_local(block).0
    }
}
