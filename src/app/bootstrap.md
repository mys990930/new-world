# bootstrap

## Role

- Create and connect the top-level runtime modules needed at app startup

## Responsibilities

- Prepare `AppConfig`
- Create `Platform`
- Load the default `BlockRegistry`
- Create `Renderer`
- Load block textures into the renderer
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
3. Load the default block registry from `assets/blocks/index.toml`
4. Create `Renderer` from a `StubSurfaceTarget` because the OS window does not exist yet
5. Convert registry texture tiles into renderer texture DTOs and call `Renderer::set_block_textures(...)`
6. Create `EcsRuntime`
7. Create `WorldCore` with the initial world seed/version metadata and shared registry
8. Create `JobSystem`
9. Spawn the default local player entity
10. Create app timing state
11. Return `GameApp`

## Output

- Initialized `GameApp`

## Invariants

- During bootstrap, the renderer may exist without a live GPU surface backend
- Bootstrap fails fast if the default block registry or block texture set cannot be loaded
- The default local player is spawned once during bootstrap
- The bootstrap local player starts at body-center `[3.0, 1.5, 3.0]` so the unit cube stands on top of the generated chunk patch

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
- The current bootstrap path is also where world-side `TextureTileSource` values are translated into renderer-side `RenderTextureSource` values.
