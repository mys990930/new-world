# frame

## Role

- Define and execute the app frame update pipeline.

## Responsibilities

- inject app timing state as ECS frame delta
- accumulate and execute app-owned fixed ticks before frame-phase gameplay work
- handle app-owned screen shortcuts before gameplay logic
- bridge platform snapshot into ECS input
- run ECS `pre/update/post` phases
- collect completed jobs and apply them to ECS/world/renderer
- collect completed minimap rebuild jobs and patch app-owned minimap cache
- run world-aware local-player motion against `WorldCore`
- plan chunk acquisition / meshing / unload lifecycle from ECS chunk state
- apply chunk unloads to world/renderer/minimap before submitting new jobs
- request minimap chunk-column rebuilds when chunk load/generate results change loaded world data
- refresh the ECS-owned local environment snapshot from the latest world state
- update world-and-viewport-based selection state
- log the clicked block key when a click lands on the current raycast target
- log chunk load/unload transitions when runtime world residency actually changes
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
- updated local environment HUD snapshot
- updated app-owned minimap cache
- optional console logging for clicked blocks and chunk residency transitions
- one renderer frame attempt

## Process

1. inject `frame_dt` into ECS
2. execute app-owned fixed ticks from the accumulated frame delta
3. handle app-owned screen shortcuts
4. if gameplay is inactive, only collect completed jobs and stop before ECS/world gameplay work
5. `bridge_platform_to_ecs()`
6. `ecs.run_pre_update()`
7. `ecs.run_update()`
8. collect any already-completed jobs into ECS/world/renderer/minimap cache
9. run `ecs.simulate_local_player_motion(&world)` so player collision uses the current world source of truth
10. `ecs.run_post_update()`
11. plan chunk lifecycle with `ecs.plan_chunk_lifecycle(&world, created_world.as_ref())`
12. apply unload coords to `WorldCore`, renderer chunk meshes, and minimap cache
13. submit the planned jobs
14. collect newly completed jobs again
15. refresh the ECS-local environment snapshot from the latest player transform and world environment state
16. update `SelectionState` from the latest world state and viewport
17. if left/right click happened and the current selection is valid, log the clicked block key/id/coord to the console
18. drain discrete commands without per-frame debug output
19. build render DTOs, including app-owned sprite UI data, and call `renderer.render(...)`

## Invariants

- world-aware player motion happens after job results are applied and before camera follow runs in `post_update`
- local-environment refresh and selection update both happen after world/job result application
- minimap viewport composition must read app-owned cached data only; completed jobs and future local world edits are the only sources that mutate the cache
- unloads happen before new frame job submission so stale load/mesh work has a clear acceptance gate
- block logging is click-triggered so the console does not flood every frame
- chunk load/unload logging is tied to actual residency changes, not to every lifecycle plan recomputation
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
- the current fixed slice is intentionally narrow: world calendar, per-atlas climate drift, local weather windows, and renderer environment sync now advance on fixed ticks, while direct simulation-driven `WorldEdit` application remains a later step
- the current world-select screen is a mouse-driven app-mode that skips gameplay updates, still collects completed jobs, and renders only app-owned pixel-sprite UI including a blocking loading popup while app-owned create-world work is pending
- the current inventory / quickslot HUD remains in normal `InGame` mode and is rendered as ECS-derived pixel-atlas UI over the scene
- the current minimap overlay now also shows a player-local environment status block sourced from ECS, not from app-side world sampling
- the current minimap no longer scans `WorldCore` every frame; it composes a one-chunk viewport from cached chunk-column top-down data rebuilt through jobs
- steady-state chunk lifetime now supports interest-vs-retain hysteresis and app-owned unload application
