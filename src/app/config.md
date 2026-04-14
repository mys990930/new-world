# config

## Role

- Define typed app startup configuration.
- Keep frame pacing and baked-world boot policy out of the runtime loop code.

## Owned Data

### AppConfig
- window title
- window width / height
- `TimingConfig`
- `preferred_baked_world_root: Option<PathBuf>`
- `baked_worlds_dir: Option<PathBuf>`

### TimingConfig
- `target_frame_rate: Option<u32>`

## Inputs

- hardcoded defaults
- future CLI / file / environment overrides

## Outputs

- typed bootstrap inputs
- frame pacing policy for `runner.rs`
- baked-world preferred-root and auto-detection policy for `bootstrap.rs`

## State Transition Rules

- config is treated as immutable after bootstrap
- fast-changing runtime values belong in `AppTimingState`, not here

## Invariants

- `target_frame_rate = Some(n)` is only valid for `n > 0`
- `target_frame_rate = None` means uncapped frame cadence
- `preferred_baked_world_root = Some(path)` means bootstrap should try that baked world root before scanning the baked worlds directory
- `baked_worlds_dir = Some(path)` means bootstrap may scan that directory for the latest baked world root
- `baked_worlds_dir = None` disables baked-world auto-detection

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
- the current default preferred baked world is `target/world-bake/runtime_seed_42_cx5_cz-8_r6_v10`
- the current default baked-world scan directory is `target/world-bake`
