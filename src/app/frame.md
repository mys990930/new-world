# frame

## Role

- Define and execute the app frame update pipeline.

## Responsibilities

- inject app timing state as ECS frame delta
- handle app-owned screen shortcuts before gameplay logic
- bridge platform snapshot into ECS input
- run ECS `pre/update/post` phases
- collect completed jobs and apply them to ECS/world/renderer
- run world-aware local-player motion against `WorldCore`
- plan chunk acquisition / meshing jobs from ECS chunk state
- update world-and-viewport-based selection state
- log the clicked block key when a click lands on the current raycast target
- drain discrete commands for debugging
- build render-ready frame DTOs, including atlas-backed UI sprites, and call the renderer

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
2. handle app-owned screen shortcuts
3. if gameplay is inactive, only collect completed jobs and stop before ECS/world gameplay work
4. `bridge_platform_to_ecs()`
5. `ecs.run_pre_update()`
6. `ecs.run_update()`
7. collect any already-completed jobs into ECS/world/renderer
8. run `ecs.simulate_local_player_motion(&world)` so player collision uses the current world source of truth
9. `ecs.run_post_update()`
10. plan chunk requests with `ecs.plan_chunk_job_requests(&world, created_world.as_ref())`
11. submit the planned jobs
12. collect newly completed jobs again
13. update `SelectionState` from the latest world state and viewport
14. if left/right click happened and the current selection is valid, log the clicked block key/id/coord to the console
15. drain and optionally log discrete commands
16. build render DTOs, including app-owned sprite UI data, and call `renderer.render(...)`

## Invariants

- world-aware player motion happens after job results are applied and before camera follow runs in `post_update`
- selection update happens after world/job result application
- block logging is click-triggered so the console does not flood every frame
- renderer receives render-ready DTOs only
- app-owned screen modes may suspend gameplay updates without changing renderer ownership boundaries
- world-select create/load actions stay app-owned; gameplay update suspension does not hand world ownership to renderer UI

## Related Modules

- `bridge.rs`
- `runner.rs`
- `ecs`
- `world`
- `renderer`

## Notes

- the current minimal chunk path now supports `LoadChunk -> BuildChunkMesh -> RenderUploadRequest` when a created world is available, and `GenerateChunk -> BuildChunkMesh -> RenderUploadRequest` as fallback
- the current player motion slice supports `2x2x4` body collision, one-block step-up, and gravity/falling against loaded world blocks
- the current world-select screen is a mouse-driven app-mode that skips gameplay updates, still collects completed jobs, and renders only app-owned pixel-sprite UI
