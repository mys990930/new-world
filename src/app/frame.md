# frame

## Role

- Define and execute the app frame update pipeline.

## Responsibilities

- inject app timing state as ECS frame delta
- bridge platform snapshot into ECS input
- run ECS `pre/update/post` phases
- collect completed jobs and apply them to ECS/world/renderer
- run world-aware local-player motion against `WorldCore`
- plan chunk acquisition / meshing jobs from ECS chunk state
- update world-and-viewport-based selection state
- drain discrete commands for debugging
- build render-ready frame DTOs and call the renderer

## Non-Responsibilities

- computing frame cap deadlines
- fixed timestep accumulation
- simulation stepping
- low-level renderer draw implementation

## Inputs

- current platform snapshot
- current frame timing state
- current world/job results
- current viewport

## Outputs

- updated ECS state
- updated world chunk state
- updated renderer chunk cache
- updated `SelectionState`
- optional debug logging for discrete commands
- one renderer frame attempt

## Process

1. inject `frame_dt` into ECS
2. `bridge_platform_to_ecs()`
3. `ecs.run_pre_update()`
4. `ecs.run_update()`
5. collect any already-completed jobs into ECS/world/renderer
6. run `ecs.simulate_local_player_motion(&world)` so player collision uses the current world source of truth
7. `ecs.run_post_update()`
8. plan chunk requests with `ecs.plan_chunk_job_requests(&world, baked_world.as_ref())`
9. submit the planned jobs
10. collect newly completed jobs again
11. update `SelectionState` from the latest world state and viewport
12. drain and optionally log discrete commands
13. build render DTOs and call `renderer.render(...)`

## Invariants

- world-aware player motion happens after job results are applied and before camera follow runs in `post_update`
- selection update happens after world/job result application
- renderer receives render-ready DTOs only

## Related Modules

- `bridge.rs`
- `runner.rs`
- `ecs`
- `world`
- `renderer`

## Notes

- the current minimal chunk path now supports `LoadChunk -> BuildChunkMesh -> RenderUploadRequest` when a baked world is available, and `GenerateChunk -> BuildChunkMesh -> RenderUploadRequest` as fallback
- the current player motion slice supports `2x2x4` body collision, one-block step-up, and gravity/falling against loaded world blocks
