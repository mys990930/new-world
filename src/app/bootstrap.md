# bootstrap

## Role

- Create and connect the top-level runtime modules needed at app startup

## Responsibilities

- Prepare `AppConfig`
- Create `Platform`
- Create `Renderer`
- Create `EcsRuntime`
- Spawn the default local player
- Create `AppTimingState`
- Assemble `GameApp`

## Non-Responsibilities

- Running the main loop
- Scheduling redraws
- Stepping fixed ticks
- Applying gameplay rules

## Process

1. Read config inputs
2. Create `Platform`
3. Create `Renderer` from a `StubSurfaceTarget` because the OS window does not exist yet
4. Create `EcsRuntime`
5. Spawn the default local player entity
6. Create app timing state
7. Return `GameApp`

## Output

- Initialized `GameApp`

## Invariants

- During bootstrap, the renderer may exist without a live GPU surface backend
- The default local player is spawned once during bootstrap
- The bootstrap local player starts at body-center `[0.0, 0.5, 0.0]` so the current unit cube prototype stands on the ground plane

## Related Modules

- `config.rs`
- `state.rs`
- `platform`
- `renderer`
- `ecs`

## Notes

- Live window surface attachment happens later in [`runner.rs`](C:/dev/new-world/src/app/runner.rs), during `resumed()`
