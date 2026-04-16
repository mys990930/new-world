# frame

## Role

- Define and execute the app frame update pipeline.

## Responsibilities

- inject app timing state as ECS frame delta
- handle app-owned screen shortcuts before gameplay logic
- bridge platform snapshot into ECS input
- run ECS `pre/update/post` phases
- collect completed jobs and apply them to ECS/world/renderer
- collect completed minimap rebuild jobs and patch app-owned minimap cache
- run world-aware local-player motion against `WorldCore`
- plan chunk acquisition / meshing / unload lifecycle from ECS chunk state
- apply chunk unloads to world/renderer/minimap before submitting new jobs
- request minimap chunk-column rebuilds when chunk load/generate results change loaded world data
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
- updated app-owned minimap cache
- optional debug logging for discrete commands
- one renderer frame attempt

## Process

1. inject `frame_dt` into ECS
2. handle app-owned screen shortcuts
3. if gameplay is inactive, only collect completed jobs and stop before ECS/world gameplay work
4. `bridge_platform_to_ecs()`
5. `ecs.run_pre_update()`
6. `ecs.run_update()`
7. collect any already-completed jobs into ECS/world/renderer/minimap cache
8. run `ecs.simulate_local_player_motion(&world)` so player collision uses the current world source of truth
9. `ecs.run_post_update()`
10. plan chunk lifecycle with `ecs.plan_chunk_lifecycle(&world, created_world.as_ref())`
11. apply unload coords to `WorldCore`, renderer chunk meshes, and minimap cache
12. submit the planned jobs
13. collect newly completed jobs again
14. update `SelectionState` from the latest world state and viewport
15. if left/right click happened and the current selection is valid, log the clicked block key/id/coord to the console
16. drain and optionally log discrete commands
17. build render DTOs, including app-owned sprite UI data, and call `renderer.render(...)`

## Invariants

- world-aware player motion happens after job results are applied and before camera follow runs in `post_update`
- selection update happens after world/job result application
- minimap viewport composition must read app-owned cached data only; completed jobs and future local world edits are the only sources that mutate the cache
- unloads happen before new frame job submission so stale load/mesh work has a clear acceptance gate
- block logging is click-triggered so the console does not flood every frame
- renderer receives render-ready DTOs only
- app-owned screen modes may suspend gameplay updates without changing renderer ownership boundaries
- world-select create/load actions stay app-owned; gameplay update suspension does not hand world ownership to renderer UI
- inventory-open gameplay blocking is handled inside ECS/player state, not by suspending the whole app frame

## Related Modules

- `bridge.rs`
- `runner.rs`
- `ecs`
- `world`
- `renderer`

## Notes

- the current minimal chunk path now supports `LoadChunk -> BuildChunkMesh -> RenderUploadRequest` when a created world is available, and `GenerateChunk -> BuildChunkMesh -> RenderUploadRequest` as fallback
- the current player motion slice supports `2x2x4` body collision, one-block step-up, and gravity/falling against loaded world blocks
- the current world-select screen is a mouse-driven app-mode that skips gameplay updates, still collects completed jobs, and renders only app-owned pixel-sprite UI including a blocking loading popup while app-owned create-world work is pending
- the current inventory / quickslot HUD remains in normal `InGame` mode and is rendered as ECS-derived pixel-atlas UI over the scene
- the current minimap no longer scans `WorldCore` every frame; it composes a one-chunk viewport from cached chunk-column top-down data rebuilt through jobs
- steady-state chunk lifetime now supports interest-vs-retain hysteresis and app-owned unload application
