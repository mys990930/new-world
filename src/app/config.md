# config

## Role

- Define typed app startup configuration.
- Keep frame pacing and created-world boot policy out of the runtime loop code.

## Owned Data

### AppConfig
- window title
- window width / height
- `TimingConfig`
- `preferred_created_world_root: Option<PathBuf>`
- `created_worlds_dir: Option<PathBuf>`
- `auto_open_latest_created_world: bool`

### TimingConfig
- `target_frame_rate: Option<u32>`
- `fixed_tick_rate: u32`
- `max_fixed_steps_per_frame: u32`

## Inputs

- hardcoded defaults
- future CLI / file / environment overrides

## Outputs

- typed bootstrap inputs
- frame pacing policy for `runner.rs`
- fixed-step pacing policy for `fixed.rs`
- created-world preferred-root and auto-detection policy for `bootstrap.rs`

## State Transition Rules

- config is treated as immutable after bootstrap
- fast-changing runtime values belong in `AppTimingState`, not here

## Invariants

- `target_frame_rate = Some(n)` is only valid for `n > 0`
- `target_frame_rate = None` means uncapped frame cadence
- `fixed_tick_rate` must be greater than zero
- `max_fixed_steps_per_frame` bounds catch-up work
- `preferred_created_world_root = Some(path)` means bootstrap should try that created world root before applying any latest-world auto-open policy
- `created_worlds_dir = Some(path)` means app UI may scan that directory for created-world choices
- `created_worlds_dir = None` disables created-world menu discovery and latest-world auto-open
- `auto_open_latest_created_world = true` means bootstrap may scan `created_worlds_dir` and open the latest created world before the menu

## Non-Responsibilities

- parsing config files
- validating UI-facing settings
- driving the event loop

## Related Modules

- `bootstrap.rs`
- `state.rs`
- `runner.rs`

## Notes

- the current default frame cap is `60 FPS`
- the current default fixed rate is `20 Hz`
- the current default `max_fixed_steps_per_frame` is `4`
- the current default preferred created world is `None`; the main binary should not hardcode a sample created-world root
- the current default created-world scan directory is `target/world-create`
- the current default `auto_open_latest_created_world` is `false`, so the main binary starts at the world-select screen even when created worlds already exist
