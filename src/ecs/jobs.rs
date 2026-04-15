use crate::jobs::{JobRequest, JobResult};
use crate::world::{CreatedWorldSource, ChunkCoord, WorldBlockCoord, WorldCore};

use super::{ChunkStates, EcsRuntime, HORIZONTAL_INTEREST_CHUNK_RADIUS};

impl EcsRuntime {
    pub fn plan_chunk_job_requests(
        &mut self,
        world: &WorldCore,
        created_world: Option<&CreatedWorldSource>,
    ) -> Vec<JobRequest> {
        let target_chunk = self.focused_player_chunk();
        let interest = interest_coords(target_chunk, created_world);
        let mut chunk_states = self.world_mut().resource_mut::<ChunkStates>();
        chunk_states.set_interest(interest.clone());
        sync_loaded_chunk_states(&mut chunk_states, world);

        let mut requests = Vec::new();
        for coord in &interest {
            if chunk_states.loaded.contains(coord) {
                continue;
            }

            if chunk_states.load_requested.contains(coord)
                || chunk_states.generation_requested.contains(coord)
            {
                continue;
            }

            if created_world.is_some_and(|source| source.contains_chunk(*coord)) {
                let root = created_world
                    .expect("contains_chunk check must imply created world exists")
                    .root()
                    .to_path_buf();
                requests.push(JobRequest::LoadChunk { root, coord: *coord });
                chunk_states.load_requested.insert(*coord);
                continue;
            }

            requests.push(JobRequest::GenerateChunk {
                coord: *coord,
                meta: *world.meta(),
                registry: world.block_registry_handle(),
            });
            chunk_states.generation_requested.insert(*coord);
        }

        enqueue_mesh_requests_for_interest(
            &mut requests,
            &interest,
            &mut chunk_states,
            world,
        );

        requests
    }

    pub fn apply_job_result(&mut self, result: &JobResult) {
        let mut chunk_states = self.world_mut().resource_mut::<ChunkStates>();

        match result {
            JobResult::ChunkLoaded { coord, .. } => {
                chunk_states.load_requested.remove(coord);
                chunk_states.loaded.insert(*coord);
            }
            JobResult::ChunkGenerated { coord, .. } => {
                chunk_states.generation_requested.remove(coord);
                chunk_states.loaded.insert(*coord);
            }
            JobResult::ChunkMeshBuilt { coord, .. } => {
                chunk_states.mesh_requested.remove(coord);
                chunk_states.render_ready.insert(*coord);
            }
            JobResult::WorldCreated { .. } => {}
            JobResult::JobFailed { request, .. } => match request {
                JobRequest::CreateWorld { .. } => {}
                JobRequest::LoadChunk { coord, .. } => {
                    chunk_states.load_requested.remove(coord);
                }
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
            .unwrap_or([16.0, 3.0, 16.0]);
        let block = WorldBlockCoord(
            world[0].floor() as i32,
            world[1].floor() as i32,
            world[2].floor() as i32,
        );
        crate::world::world_to_chunk_local(block).0
    }
}

fn sync_loaded_chunk_states(chunk_states: &mut ChunkStates, world: &WorldCore) {
    chunk_states.loaded.retain(|coord| world.has_chunk(*coord));
    chunk_states.load_requested.retain(|coord| !world.has_chunk(*coord));
    chunk_states
        .generation_requested
        .retain(|coord| !world.has_chunk(*coord));
    chunk_states.render_ready.retain(|coord| world.has_chunk(*coord));

    let interest: Vec<_> = chunk_states.interest.iter().copied().collect();
    for coord in interest {
        if world.has_chunk(coord) {
            chunk_states.loaded.insert(coord);
        }
    }
}

fn enqueue_mesh_requests_for_interest(
    requests: &mut Vec<JobRequest>,
    interest: &[ChunkCoord],
    chunk_states: &mut ChunkStates,
    world: &WorldCore,
) {
    for &coord in interest {
        if !chunk_states.loaded.contains(&coord)
            || chunk_states.render_ready.contains(&coord)
            || chunk_states.mesh_requested.contains(&coord)
        {
            continue;
        }

        let Some(center) = world.snapshot_chunk(coord) else {
            continue;
        };

        requests.push(JobRequest::BuildChunkMesh {
            center,
            neighbors: world.query_neighbors(coord),
            registry: world.block_registry_handle(),
        });
        chunk_states.mesh_requested.insert(coord);
    }
}

fn interest_coords(
    target_chunk: ChunkCoord,
    created_world: Option<&CreatedWorldSource>,
) -> Vec<ChunkCoord> {
    let mut coords = Vec::new();
    let radius = HORIZONTAL_INTEREST_CHUNK_RADIUS;

    for z in (target_chunk.2 - radius)..=(target_chunk.2 + radius) {
        for x in (target_chunk.0 - radius)..=(target_chunk.0 + radius) {
            if let Some(source) = created_world {
                let min = source.manifest().min_chunk_coord();
                let max = source.manifest().max_chunk_coord();
                for y in min.1..=max.1 {
                    let coord = ChunkCoord(x, y, z);
                    if source.contains_chunk(coord) {
                        coords.push(coord);
                    }
                }
            } else {
                coords.push(ChunkCoord(x, target_chunk.1, z));
            }
        }
    }

    coords
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::world::{BlockRegistry, ChunkData, WorldMeta};

    fn test_registry() -> Arc<BlockRegistry> {
        Arc::new(BlockRegistry::load_default().expect("default registry should load"))
    }

    fn test_world() -> WorldCore {
        WorldCore::new(WorldMeta::default(), test_registry())
    }

    #[test]
    fn enqueue_mesh_requests_includes_loaded_vertical_interest_chunks() {
        let mut world = test_world();
        let surface = ChunkCoord(0, 0, 0);
        let below = ChunkCoord(0, -1, 0);
        world.insert_chunk(surface, ChunkData::new_empty(surface));
        world.insert_chunk(below, ChunkData::new_empty(below));

        let mut requests = Vec::new();
        let mut chunk_states = ChunkStates::default();
        chunk_states.loaded.insert(surface);
        chunk_states.loaded.insert(below);

        enqueue_mesh_requests_for_interest(
            &mut requests,
            &[surface, below],
            &mut chunk_states,
            &world,
        );

        assert_eq!(requests.len(), 2);
        assert!(requests.iter().any(|request| request.coord() == surface));
        assert!(requests.iter().any(|request| request.coord() == below));
        assert!(chunk_states.mesh_requested.contains(&surface));
        assert!(chunk_states.mesh_requested.contains(&below));
    }

    #[test]
    fn enqueue_mesh_requests_skips_render_ready_and_already_requested_chunks() {
        let mut world = test_world();
        let ready = ChunkCoord(0, 0, 0);
        let pending = ChunkCoord(0, -1, 0);
        let fresh = ChunkCoord(0, -2, 0);
        world.insert_chunk(ready, ChunkData::new_empty(ready));
        world.insert_chunk(pending, ChunkData::new_empty(pending));
        world.insert_chunk(fresh, ChunkData::new_empty(fresh));

        let mut requests = Vec::new();
        let mut chunk_states = ChunkStates::default();
        chunk_states.loaded.extend([ready, pending, fresh]);
        chunk_states.render_ready.insert(ready);
        chunk_states.mesh_requested.insert(pending);

        enqueue_mesh_requests_for_interest(
            &mut requests,
            &[ready, pending, fresh],
            &mut chunk_states,
            &world,
        );

        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].coord(), fresh);
        assert!(chunk_states.mesh_requested.contains(&fresh));
    }
}
