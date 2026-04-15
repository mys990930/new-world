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
- `created_world`
- `jobs`
- `renderer`
- `ui`
- `timing`

### AppTimingState
- `frame_index`
- `frame_dt`
- `last_frame_instant`
- `next_frame_deadline`

## Inputs

- bootstrap-created module instances
- current time from the runner
- frame pacing policy from config

## Outputs

- shared state for `runner.rs`, `frame.rs`, and future `fixed.rs`

## State Transition Rules

- module instances are created during bootstrap and then owned by `GameApp`
- `frame_index` and `frame_dt` update only when a frame actually runs
- `next_frame_deadline` is only meaningful while a frame cap is active

## Invariants

- `frame_dt` is frame cadence state, not fixed-tick state
- `created_world` holds runtime metadata about the currently selected auto-detected created world, if one exists

## Non-Responsibilities

- raw input capture
- gameplay rule evaluation
- world source-of-truth mutation policy

## Related Modules

- `bootstrap.rs`
- `runner.rs`
- `frame.rs`

## Notes

- the current `GameApp` always owns exactly one active `Platform`, `EcsRuntime`, `WorldCore`, `JobSystem`, and `Renderer`
- created-world runtime ownership lives in app state because `app` decides whether world acquisition should load from disk or fall back to generation
- top-level screen mode and lightweight overlay visibility are also app-owned because they should not force ECS/world dependencies
