# bootstrap

## Role

- Create and connect the top-level runtime modules needed at app startup

## Responsibilities

- Prepare `AppConfig`
- Create `Platform`
- Create `Renderer`
- Create `EcsRuntime`
- Create `WorldCore`
- Create `JobSystem`
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
5. Create `WorldCore` with the initial world seed/version metadata
6. Create `JobSystem`
7. Spawn the default local player entity
8. Create app timing state
9. Return `GameApp`

## Output

- Initialized `GameApp`

## Invariants

- During bootstrap, the renderer may exist without a live GPU surface backend
- The default local player is spawned once during bootstrap
- The bootstrap local player starts at body-center `[8.0, 1.5, 8.0]` so the unit cube stands on top of the generated chunk plane

## Related Modules

- `config.rs`
- `state.rs`
- `platform`
- `world`
- `jobs`
- `renderer`
- `ecs`

## Notes

- Live window surface attachment happens later in [`runner.rs`](C:/dev/new-world/src/app/runner.rs), during `resumed()`
