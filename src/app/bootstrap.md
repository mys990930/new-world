# bootstrap

## Role

- Create and connect the top-level runtime modules needed at app startup.

## Responsibilities

- prepare `AppConfig`
- create `Platform`
- load the default `BlockRegistry`
- create `Renderer`
- load block textures into the renderer
- open the configured preferred baked world when available, otherwise auto-detect the latest baked world
- create `EcsRuntime`
- create `WorldCore`
- preload the initial spawn neighborhood into memory
- spawn the default local player
- place the player on a safe surface when preload data is available
- create `JobSystem`
- create `AppTimingState`
- assemble `GameApp`

## Non-Responsibilities

- running the main loop
- scheduling redraws
- stepping fixed ticks
- applying gameplay rules every frame

## Process

1. read config inputs
2. create `Platform`
3. load the default block registry from `assets/blocks/index.toml`
4. create `Renderer` from a `StubSurfaceTarget` because the OS window does not exist yet
5. convert registry texture tiles into renderer texture DTOs and call `Renderer::set_block_textures(...)`
6. try `AppConfig.preferred_baked_world_root` first, then auto-detect the latest baked world root under `AppConfig.baked_worlds_dir` if needed
7. create `EcsRuntime`
8. create `WorldCore` using baked manifest metadata when a baked world exists, otherwise use fallback procedural metadata
9. preload the spawn neighborhood
10. spawn the default local player entity
11. snap the local player to a safe loaded surface near the preload anchor when possible
12. create `JobSystem`
13. create app timing state
14. return `GameApp`

## Output

- initialized `GameApp`

## Invariants

- during bootstrap, the renderer may exist without a live GPU surface backend
- bootstrap fails fast if the default block registry or block texture set cannot be loaded
- the default local player is spawned once during bootstrap
- if preload data exists, bootstrap attempts to place the player so the `2x2x4` body does not start embedded in solid blocks

## Related Modules

- `config.rs`
- `state.rs`
- `platform`
- `world`
- `jobs`
- `renderer`
- `ecs`

## Notes

- live window surface attachment still happens later in [runner.rs](/C:/dev/new-world/src/app/runner.rs)
- the current bootstrap path also translates world-side `TextureTileSource` values into renderer-side `RenderTextureSource`
- bootstrap now aligns the renderer camera FOV with the ECS weak-perspective quarter-view constant so render projection and selection ray construction stay in sync
- when a baked world is found, bootstrap preloads a `5x5` horizontal neighborhood of baked chunk columns around the baked preview chunk so spawn placement and first-frame movement do not expose chunk edges immediately
- when no baked world is found, bootstrap falls back to generating a small procedural `5x5` neighborhood on the player plane
