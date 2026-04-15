# bootstrap

## Role

- Create and connect the top-level runtime modules needed at app startup.

## Responsibilities

- prepare `AppConfig`
- create `Platform`
- load the default `BlockRegistry`
- create `Renderer`
- load block textures into the renderer
- load the pixel UI atlas into the renderer
- open the configured preferred created world when available, otherwise auto-detect the latest created world
- create `EcsRuntime`
- create `WorldCore`
- preload the initial spawn neighborhood into memory
- spawn the default local player
- place the player on a safe surface when preload data is available
- create `JobSystem`
- create `AppTimingState`
- assemble `GameApp`
- provide app-owned create-world request / created-world reload helpers for the world-select screen

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
6. load the pixel UI atlas into the renderer so app-owned overlays can render through sprite DTOs
7. try `AppConfig.preferred_created_world_root` first, then auto-detect the latest created world root under `AppConfig.created_worlds_dir` if needed
8. create `EcsRuntime`
9. create `WorldCore` using created-world manifest metadata when a created world exists, otherwise use fallback procedural metadata
10. preload the spawn neighborhood
11. spawn the default local player entity
12. snap the local player to a safe loaded surface near the preload anchor when possible
13. create `JobSystem`
14. create app timing state
15. return `GameApp`

## Output

- initialized `GameApp`

## Invariants

- during bootstrap, the renderer may exist without a live GPU surface backend
- bootstrap fails fast if the default block registry or block texture set cannot be loaded
- UI atlas load is best-effort; renderer can fall back to a white dummy texture if the atlas file is missing
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
- when a created world is found, bootstrap preloads a `5x5` horizontal neighborhood of created-world chunk columns around the created-world preview chunk so spawn placement and first-frame movement do not expose chunk edges immediately
- when no created world is found, bootstrap falls back to generating a small procedural `5x5` neighborhood on the player plane
- the current app-owned world-select screen reuses bootstrap-style helpers to queue create-world work and reload the runtime into a newly selected created-world root without changing lower-layer ownership
