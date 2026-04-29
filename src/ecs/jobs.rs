use crate::jobs::{JobRequest, JobResult};
use crate::world::{ChunkCoord, CreatedWorldSource, WorldBlockCoord, WorldCore};

use super::{
    ChunkLifecyclePlan, ChunkStates, EcsRuntime, HORIZONTAL_INTEREST_CHUNK_RADIUS,
    HORIZONTAL_RETAIN_CHUNK_RADIUS,
};

const MAX_CHUNK_ACQUISITION_REQUESTS_PER_FRAME: usize = 8;
const MAX_CHUNK_MESH_REQUESTS_PER_FRAME: usize = 4;

impl EcsRuntime {
    pub fn plan_chunk_lifecycle(
        &mut self,
        world: &WorldCore,
        created_world: Option<&CreatedWorldSource>,
    ) -> ChunkLifecyclePlan {
        let target_chunk = self.focused_player_chunk();
        let interest = envelope_coords(
            target_chunk,
            created_world,
            HORIZONTAL_INTEREST_CHUNK_RADIUS,
        );
        let retain = envelope_coords(target_chunk, created_world, HORIZONTAL_RETAIN_CHUNK_RADIUS);
        let mut chunk_states = self.world_mut().resource_mut::<ChunkStates>();
        sync_loaded_chunk_states(&mut chunk_states, world);
        chunk_states.set_interest(interest.iter().copied());
        chunk_states.set_retain(retain.iter().copied());
        let unload_coords = planned_unload_coords(&chunk_states, &retain);

        let mut requests = Vec::new();
        let ordered_interest = ordered_interest_coords(&interest, target_chunk);
        let mut acquisition_requests = 0_usize;
        for coord in &ordered_interest {
            if acquisition_requests >= MAX_CHUNK_ACQUISITION_REQUESTS_PER_FRAME {
                break;
            }

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
                requests.push(JobRequest::LoadChunk {
                    root,
                    coord: *coord,
                });
                chunk_states.load_requested.insert(*coord);
                acquisition_requests += 1;
                continue;
            }

            requests.push(JobRequest::GenerateChunk {
                coord: *coord,
                meta: *world.meta(),
                registry: world.block_registry_handle(),
            });
            chunk_states.generation_requested.insert(*coord);
            acquisition_requests += 1;
        }

        enqueue_mesh_requests_for_interest(
            &mut requests,
            &ordered_interest,
            &mut chunk_states,
            world,
        );

        ChunkLifecyclePlan {
            interest,
            retain,
            job_requests: requests,
            unload_coords,
        }
    }

    pub fn apply_job_result(&mut self, result: &JobResult) {
        let mut chunk_states = self.world_mut().resource_mut::<ChunkStates>();

        match result {
            JobResult::CreateWorldProgress { .. } => {}
            JobResult::ChunkLoaded { coord, .. } => {
                chunk_states.load_requested.remove(coord);
                if chunk_states.retain.contains(coord) {
                    chunk_states.loaded.insert(*coord);
                    chunk_states.render_ready.remove(coord);
                    chunk_states.remesh_needed.remove(coord);
                    invalidate_loaded_neighbor_meshes(&mut chunk_states, *coord);
                } else {
                    chunk_states.loaded.remove(coord);
                    chunk_states.render_ready.remove(coord);
                    chunk_states.remesh_needed.remove(coord);
                }
            }
            JobResult::ChunkGenerated { coord, .. } => {
                chunk_states.generation_requested.remove(coord);
                if chunk_states.retain.contains(coord) {
                    chunk_states.loaded.insert(*coord);
                    chunk_states.render_ready.remove(coord);
                    chunk_states.remesh_needed.remove(coord);
                    invalidate_loaded_neighbor_meshes(&mut chunk_states, *coord);
                } else {
                    chunk_states.loaded.remove(coord);
                    chunk_states.render_ready.remove(coord);
                    chunk_states.remesh_needed.remove(coord);
                }
            }
            JobResult::ChunkMeshBuilt { coord, .. } => {
                chunk_states.mesh_requested.remove(coord);
                if chunk_states.retain.contains(coord) {
                    chunk_states.render_ready.insert(*coord);
                } else {
                    chunk_states.render_ready.remove(coord);
                }
            }
            JobResult::MinimapChunkColumnBuilt { .. } => {}
            JobResult::RegionClassResolved { .. } => {}
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
                JobRequest::BuildMinimapChunkColumn { .. } => {}
                JobRequest::ResolveRegionClassArea { .. } => {}
            },
        }
    }

    pub fn apply_chunk_unloaded(&mut self, coord: ChunkCoord) {
        let mut chunk_states = self.world_mut().resource_mut::<ChunkStates>();
        chunk_states.interest.remove(&coord);
        chunk_states.retain.remove(&coord);
        chunk_states.loaded.remove(&coord);
        chunk_states.load_requested.remove(&coord);
        chunk_states.generation_requested.remove(&coord);
        chunk_states.mesh_requested.remove(&coord);
        chunk_states.remesh_needed.remove(&coord);
        chunk_states.render_ready.remove(&coord);
        invalidate_loaded_neighbor_meshes(&mut chunk_states, coord);
    }

    pub fn retains_chunk(&self, coord: ChunkCoord) -> bool {
        self.world()
            .resource::<ChunkStates>()
            .retain
            .contains(&coord)
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
    chunk_states
        .load_requested
        .retain(|coord| !world.has_chunk(*coord));
    chunk_states
        .generation_requested
        .retain(|coord| !world.has_chunk(*coord));
    chunk_states
        .remesh_needed
        .retain(|coord| world.has_chunk(*coord));
    chunk_states
        .render_ready
        .retain(|coord| world.has_chunk(*coord));

    let interest: Vec<_> = chunk_states.interest.iter().copied().collect();
    for coord in interest {
        if world.has_chunk(coord) {
            chunk_states.loaded.insert(coord);
        }
    }
}

fn planned_unload_coords(
    chunk_states: &ChunkStates,
    retain: &std::collections::BTreeSet<ChunkCoord>,
) -> Vec<ChunkCoord> {
    chunk_states
        .loaded
        .iter()
        .filter(|coord| !retain.contains(coord))
        .copied()
        .collect()
}

fn ordered_interest_coords(
    interest: &std::collections::BTreeSet<ChunkCoord>,
    target_chunk: ChunkCoord,
) -> Vec<ChunkCoord> {
    let mut coords = interest.iter().copied().collect::<Vec<_>>();
    coords.sort_by_key(|coord| {
        let dx = i64::from(coord.0 - target_chunk.0);
        let dz = i64::from(coord.2 - target_chunk.2);
        let horizontal_distance_sq = dx * dx + dz * dz;
        (
            horizontal_distance_sq,
            coord.1.abs_diff(target_chunk.1),
            coord.1,
            coord.0,
            coord.2,
        )
    });
    coords
}

fn enqueue_mesh_requests_for_interest(
    requests: &mut Vec<JobRequest>,
    interest: &[ChunkCoord],
    chunk_states: &mut ChunkStates,
    world: &WorldCore,
) {
    let mut mesh_requests = 0_usize;
    for &coord in interest {
        if mesh_requests >= MAX_CHUNK_MESH_REQUESTS_PER_FRAME {
            break;
        }

        if !chunk_states.loaded.contains(&coord) || chunk_states.mesh_requested.contains(&coord) {
            continue;
        }

        let needs_mesh = !chunk_states.render_ready.contains(&coord)
            || chunk_states.remesh_needed.contains(&coord);
        if !needs_mesh {
            continue;
        }

        if has_unresolved_interest_neighbor(coord, chunk_states) {
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
        chunk_states.remesh_needed.remove(&coord);
        mesh_requests += 1;
    }
}

fn has_unresolved_interest_neighbor(coord: ChunkCoord, chunk_states: &ChunkStates) -> bool {
    adjacent_chunk_coords(coord).into_iter().any(|neighbor| {
        chunk_states.interest.contains(&neighbor) && !chunk_states.loaded.contains(&neighbor)
    })
}

fn invalidate_loaded_neighbor_meshes(chunk_states: &mut ChunkStates, coord: ChunkCoord) {
    for neighbor in adjacent_chunk_coords(coord) {
        if chunk_states.loaded.contains(&neighbor) {
            chunk_states.remesh_needed.insert(neighbor);
        }
    }
}

fn adjacent_chunk_coords(coord: ChunkCoord) -> [ChunkCoord; 6] {
    [
        coord.offset(-1, 0, 0),
        coord.offset(1, 0, 0),
        coord.offset(0, -1, 0),
        coord.offset(0, 1, 0),
        coord.offset(0, 0, -1),
        coord.offset(0, 0, 1),
    ]
}

fn envelope_coords(
    target_chunk: ChunkCoord,
    created_world: Option<&CreatedWorldSource>,
    radius: i32,
) -> std::collections::BTreeSet<ChunkCoord> {
    let mut coords = std::collections::BTreeSet::new();

    for z in (target_chunk.2 - radius)..=(target_chunk.2 + radius) {
        for x in (target_chunk.0 - radius)..=(target_chunk.0 + radius) {
            if let Some(source) = created_world {
                let min = source.manifest().min_chunk_coord();
                let max = source.manifest().max_chunk_coord();
                for y in min.1..=max.1 {
                    let coord = ChunkCoord(x, y, z);
                    if source.contains_chunk(coord) {
                        coords.insert(coord);
                    }
                }
            } else {
                coords.insert(ChunkCoord(x, target_chunk.1, z));
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

    #[test]
    fn enqueue_mesh_requests_remeshes_dirty_render_ready_chunks() {
        let mut world = test_world();
        let dirty = ChunkCoord(0, 0, 0);
        world.insert_chunk(dirty, ChunkData::new_empty(dirty));

        let mut requests = Vec::new();
        let mut chunk_states = ChunkStates::default();
        chunk_states.loaded.insert(dirty);
        chunk_states.render_ready.insert(dirty);
        chunk_states.remesh_needed.insert(dirty);

        enqueue_mesh_requests_for_interest(&mut requests, &[dirty], &mut chunk_states, &world);

        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].coord(), dirty);
        assert!(chunk_states.mesh_requested.contains(&dirty));
        assert!(!chunk_states.remesh_needed.contains(&dirty));
    }

    #[test]
    fn enqueue_mesh_requests_caps_mesh_work_per_frame() {
        let mut world = test_world();
        let chunks = [
            ChunkCoord(0, 0, 0),
            ChunkCoord(0, -1, 0),
            ChunkCoord(0, -2, 0),
            ChunkCoord(0, -3, 0),
        ];
        for coord in chunks {
            world.insert_chunk(coord, ChunkData::new_empty(coord));
        }

        let mut requests = Vec::new();
        let mut chunk_states = ChunkStates::default();
        chunk_states.loaded.extend(chunks);

        enqueue_mesh_requests_for_interest(&mut requests, &chunks, &mut chunk_states, &world);

        assert_eq!(requests.len(), MAX_CHUNK_MESH_REQUESTS_PER_FRAME);
    }

    #[test]
    fn enqueue_mesh_requests_waits_for_unresolved_interest_neighbors() {
        let mut world = test_world();
        let center = ChunkCoord(0, 0, 0);
        let east = ChunkCoord(1, 0, 0);
        world.insert_chunk(center, ChunkData::new_empty(center));

        let mut requests = Vec::new();
        let mut chunk_states = ChunkStates::default();
        chunk_states.interest.extend([center, east]);
        chunk_states.loaded.insert(center);

        enqueue_mesh_requests_for_interest(&mut requests, &[center], &mut chunk_states, &world);

        assert!(requests.is_empty());
        assert!(!chunk_states.mesh_requested.contains(&center));
    }

    #[test]
    fn lifecycle_plan_caps_chunk_acquisition_work_per_frame() {
        let world = test_world();
        let mut runtime = EcsRuntime::new();

        let plan = runtime.plan_chunk_lifecycle(&world, None);
        let acquisition_requests = plan
            .job_requests
            .iter()
            .filter(|request| {
                matches!(
                    request,
                    JobRequest::LoadChunk { .. } | JobRequest::GenerateChunk { .. }
                )
            })
            .count();

        assert_eq!(
            acquisition_requests,
            MAX_CHUNK_ACQUISITION_REQUESTS_PER_FRAME
        );
    }

    #[test]
    fn new_chunk_load_invalidates_loaded_neighbor_meshes() {
        let mut runtime = EcsRuntime::new();
        let center = ChunkCoord(0, 0, 0);
        let east = ChunkCoord(1, 0, 0);
        {
            let mut chunk_states = runtime.world_mut().resource_mut::<ChunkStates>();
            chunk_states.loaded.extend([center, east]);
            chunk_states.retain.extend([center, east]);
            chunk_states.render_ready.insert(east);
        }

        runtime.apply_job_result(&JobResult::ChunkGenerated {
            coord: center,
            chunk: ChunkData::new_empty(center),
        });

        let chunk_states = runtime.world().resource::<ChunkStates>();
        assert!(!chunk_states.render_ready.contains(&center));
        assert!(chunk_states.remesh_needed.contains(&east));
    }

    #[test]
    fn stale_mesh_completion_keeps_neighbor_remesh_pending() {
        let mut runtime = EcsRuntime::new();
        let coord = ChunkCoord(0, 0, 0);
        {
            let mut chunk_states = runtime.world_mut().resource_mut::<ChunkStates>();
            chunk_states.loaded.insert(coord);
            chunk_states.retain.insert(coord);
            chunk_states.mesh_requested.insert(coord);
            chunk_states.remesh_needed.insert(coord);
        }

        runtime.apply_job_result(&JobResult::ChunkMeshBuilt {
            coord,
            mesh: crate::world::CpuMesh::default(),
        });

        let chunk_states = runtime.world().resource::<ChunkStates>();
        assert!(!chunk_states.mesh_requested.contains(&coord));
        assert!(chunk_states.render_ready.contains(&coord));
        assert!(chunk_states.remesh_needed.contains(&coord));
    }

    #[test]
    fn lifecycle_plan_unloads_chunks_outside_retain_envelope() {
        let mut world = test_world();
        let mut runtime = EcsRuntime::new();
        let far = ChunkCoord(5, 0, 0);
        world.insert_chunk(far, ChunkData::new_empty(far));
        {
            let mut chunk_states = runtime.world_mut().resource_mut::<ChunkStates>();
            chunk_states.loaded.insert(far);
        }

        let plan = runtime.plan_chunk_lifecycle(&world, None);

        assert!(plan.unload_coords.contains(&far));
    }

    #[test]
    fn ordered_interest_prioritizes_focused_column() {
        let interest = [
            ChunkCoord(2, 0, 0),
            ChunkCoord(0, 3, 0),
            ChunkCoord(0, 0, 0),
            ChunkCoord(1, 0, 0),
        ]
        .into_iter()
        .collect();

        let ordered = ordered_interest_coords(&interest, ChunkCoord(0, 0, 0));

        assert_eq!(ordered[0], ChunkCoord(0, 0, 0));
        assert_eq!(ordered[1], ChunkCoord(0, 3, 0));
    }

    #[test]
    fn stale_chunk_result_outside_retain_does_not_become_loaded() {
        let mut runtime = EcsRuntime::new();
        let coord = ChunkCoord(5, 0, 0);
        runtime
            .world_mut()
            .resource_mut::<ChunkStates>()
            .load_requested
            .insert(coord);

        runtime.apply_job_result(&JobResult::ChunkLoaded {
            coord,
            chunk: ChunkData::new_empty(coord),
        });

        let chunk_states = runtime.world().resource::<ChunkStates>();
        assert!(!chunk_states.loaded.contains(&coord));
        assert!(!chunk_states.render_ready.contains(&coord));
        assert!(!chunk_states.load_requested.contains(&coord));
    }
}
