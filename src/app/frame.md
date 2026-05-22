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
- collect intermediate create-world progress jobs and apply them to app-owned world-select UI state
- collect completed minimap rebuild jobs and patch app-owned minimap cache
- collect completed region-classification jobs and patch the world-owned runtime region cache
- snap a pending created-world player spawn anchor onto a safe loaded surface once streamed chunk results make that possible
- run world-aware local-player motion against `WorldCore`
- plan chunk acquisition / meshing / unload lifecycle from ECS chunk state
- submit chunk unload requests through jobs and apply completed unload results to world/renderer/minimap within the gameplay result budget
- request minimap chunk-column rebuilds when chunk load/generate results change loaded world data
- remove renderer chunk meshes for empty CPU mesh results instead of attempting an invalid empty GPU upload
- process gameplay job completions through a small per-frame budget so chunk mesh uploads cannot monopolize input frames
- refresh the ECS-owned local environment snapshot from the latest world state
- queue a focused region-classification resolve when the local environment cache is missing
- update world-and-viewport-based selection state
- log the clicked block key when a click lands on the current raycast target
- log chunk load/unload transitions when runtime world residency actually changes
- log throttled chunk lifecycle request summaries, mesh upload/skip outcomes, create-world progress, and periodic pending-spawn wait state for startup diagnosis
- optionally log update/render frame diagnostics with per-stage timings, job queue class counts, minimap cache state, and mesh upload timing
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
- current job progress snapshots
- current viewport

## Outputs

- updated ECS state
- updated world chunk state within the current frame's job-result processing budget
- updated renderer chunk cache within the current frame's job-result processing budget
- updated `SelectionState`
- updated local environment HUD snapshot
- updated app-owned minimap cache
- optional console logging for clicked blocks and chunk residency transitions
- diagnostic console logging for chunk request/result flow and pending spawned-world placement
- optional diagnostic console logging for app update/render stage timing and job/minimap pressure
- one renderer frame attempt

## Process

1. inject `frame_dt` into ECS
2. execute app-owned fixed ticks from the accumulated frame delta
3. handle app-owned screen shortcuts
4. if gameplay is inactive, only collect completed jobs and stop before ECS/world gameplay work
5. `bridge_platform_to_ecs()`
6. `ecs.run_pre_update()`
7. `ecs.run_update()`
8. collect already-completed jobs into ECS/world/renderer/minimap cache up to the gameplay result budget
9. try to place any pending created-world spawn anchor on the currently loaded surface
10. run `ecs.simulate_local_player_motion(&world)` so player collision uses the current world source of truth, unless spawn placement is still pending
11. `ecs.run_post_update()`
12. plan chunk lifecycle with `ecs.plan_chunk_lifecycle(&world, created_world.as_ref())`
13. submit the planned jobs, including `UnloadChunk` requests
14. collect newly completed jobs again if gameplay result budget remains; completed `ChunkUnloaded` results apply `WorldCore`, renderer mesh, and minimap cache removal here
15. try pending spawn placement again after same-frame job completions
16. queue a focused region-classification resolve if the player atlas cell is not cached yet
17. refresh the ECS-local environment snapshot from the latest player transform and cached world environment state
18. update `SelectionState` from the latest world state and viewport
19. if left/right click happened and the current selection is valid, log the clicked block key/id/coord to the console
20. drain discrete commands without per-frame debug output
21. build render DTOs, including app-owned sprite UI data, and call `renderer.render(...)`

## Invariants

- world-aware player motion happens after job results are applied and before camera follow runs in `post_update`
- player motion is skipped while created-world spawn placement is still pending so missing chunks remain a loading boundary rather than a collision artifact
- local-environment refresh and selection update both happen after world/job result application
- local-environment refresh must not force uncached region classification on the main thread; uncached atlas cells are resolved by jobs and use fallback display data until cached
- gameplay frames must not apply unlimited completed chunk jobs in one update; deferred results remain queued for later frames
- minimap viewport composition must read app-owned cached data only; completed jobs and future local world edits are the only sources that mutate the cache
- unload candidates enter the same job queue and result budget as other chunk work so stale load/mesh work has a clear acceptance gate without doing synchronous unload bursts in the lifecycle plan
- block logging is click-triggered so the console does not flood every frame
- chunk load/unload logging is tied to actual residency changes, not to every lifecycle plan recomputation
- chunk lifecycle plan logging is throttled while requests are active and still emitted for unload activity so normal idle frames do not flood the console
- opt-in update/render frame diagnostics should report enough stage timing to separate ECS, jobs result application, renderer upload, minimap/region cache pressure, and draw/present cost
- renderer receives render-ready DTOs only
- app-owned screen modes may suspend gameplay updates without changing renderer ownership boundaries
- job progress events may update app-owned UI state while gameplay is suspended, but must not mutate world/ECS chunk residency
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
- empty mesh results are accepted as render-ready chunk state but remove/skip renderer mesh upload because `wgpu` buffers cannot be created from empty vertex/index arrays
- the current player motion slice supports `2x2x4` body collision, one-block step-up, and gravity/falling against loaded world blocks
- the current fixed slice is intentionally narrow: world calendar, per-atlas climate drift, local weather windows, and renderer environment sync now advance on fixed ticks, while direct simulation-driven `WorldEdit` application remains a later step
- the current world-select screen is a mouse-driven app-mode that skips gameplay updates, still collects completed jobs, and renders only app-owned pixel-sprite UI including a blocking loading popup while app-owned create-world work is pending
- create-world progress events update the world-select popup counter/progress bar before the final `WorldCreated` result arrives
- the current inventory / quickslot HUD remains in normal `InGame` mode and is rendered as ECS-derived pixel-atlas UI over the scene
- the current minimap overlay now also shows a player-local environment status block sourced from ECS, not from app-side world sampling
- the current minimap no longer scans `WorldCore` every frame; it composes a one-chunk viewport from cached chunk-column top-down data rebuilt through jobs
- steady-state chunk lifetime now supports interest-vs-retain hysteresis, job-queued unload requests, and app-owned unload application after the result is drained
- startup diagnosis logs now separate window/surface startup, lifecycle request planning, disk/generated chunk arrival, mesh upload, stale-result ignores, and delayed player placement
- gameplay job result application is budgeted per frame, which spreads bursty chunk load/mesh/minimap completions across frames instead of uploading all finished meshes at once
- update/render frame perf logs are ignored by default; `NEW_WORLD_TRACE_FRAME_LOGS=1` opts into the per-frame `[perf] update/render frame` diagnostics
- focused region classification now runs as `ResolveRegionClassArea`; worker time may appear in job diagnostics, but ECS/app environment refreshes read cached samples only and stay non-blocking on the main frame
