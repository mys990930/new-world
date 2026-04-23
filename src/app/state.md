# state

## Role

- Define app-owned top-level runtime state.
- Keep long-lived module instances and frame timing state in one place.

## Owned Data

### GameApp
- `config`
- `platform`
- `ecs`
- `world`
- `simulation`
- `created_world`
- `jobs`
- `renderer`
- `ui`
- `minimap`
- `timing`

### AppTimingState
- `frame_index`
- `frame_dt`
- `last_frame_instant`
- `next_frame_deadline`
- `fixed_accumulator`

## Inputs

- bootstrap-created module instances
- current time from the runner
- frame pacing policy from config

## Outputs

- shared state for `runner.rs`, `frame.rs`, and `fixed.rs`
- shared state for `minimap.rs` cache ownership and render-viewport composition

## State Transition Rules

- module instances are created during bootstrap and then owned by `GameApp`
- minimap cache is app-owned long-lived state alongside world/jobs/renderer so cached top-down data survives across frames
- `frame_index` and `frame_dt` update only when a frame actually runs
- `next_frame_deadline` is only meaningful while a frame cap is active
- `fixed_accumulator` stores carry-over frame time for app-owned fixed stepping

## Invariants

- `frame_dt` is frame cadence state, not fixed-tick state
- `fixed_accumulator` is app-owned orchestration state, not simulation-owned state
- `created_world` holds runtime metadata about the currently selected auto-detected created world, if one exists
- minimap cache lifetime follows the active app world/session and resets when the active world is replaced

## Non-Responsibilities

- raw input capture
- gameplay rule evaluation
- world source-of-truth mutation policy

## Related Modules

- `bootstrap.rs`
- `runner.rs`
- `frame.rs`
- `minimap.rs`

## Notes

- the current `GameApp` always owns exactly one active `Platform`, `EcsRuntime`, `WorldCore`, `SimulationCore`, `JobSystem`, and `Renderer`
- created-world runtime ownership lives in app state because `app` decides whether world acquisition should load from disk or fall back to generation
- top-level screen mode and lightweight overlay visibility are also app-owned because they should not force ECS/world dependencies
- minimap cache is also app-owned because viewport policy and cache invalidation live above `world` and below `renderer`
