# bootstrap

## Role

- Create and connect the top-level runtime modules needed at app startup.

## Responsibilities

- prepare `AppConfig`
- create `Platform`
- load the default `BlockRegistry`
- create `Renderer`
- load block textures into the renderer
- load the generated pixel UI atlas into the renderer
- open the configured preferred created world when available, and only auto-detect the latest created world when config explicitly enables that policy
- enter the app-owned world-select startup screen when no created world is available
- create `EcsRuntime`
- create `WorldCore`
- create `SimulationCore`
- spawn the default local player
- stage the player at the selected created-world spawn x/z when a created world is opened
- create `JobSystem`
- create `AppMinimapCache`
- create `AppTimingState`
- sync the initial renderer environment from world-owned calendar/weather state
- queue focused region-classification cache warmup when a created world is reloaded from the world-select screen
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
6. load `assets/ui/new_world_pixel_ui_atlas.png` into the renderer so app-owned overlays can render through sprite DTOs
7. try `AppConfig.preferred_created_world_root` first, then auto-detect the latest created world root under `AppConfig.created_worlds_dir` only when `AppConfig.auto_open_latest_created_world` is enabled
8. create `EcsRuntime`
9. create `WorldCore` using created-world manifest metadata when a created world exists, otherwise use fallback metadata without chunk realization
10. spawn the default local player entity
11. if a created world is already opened, move the player to a loading-height placeholder at the manifest preview x/z and store that spawn anchor for later surface placement
12. create `JobSystem`
13. create `SimulationCore` from the configured fixed tick rate
14. create an empty app-owned minimap cache
15. queue initial minimap rebuild work for any already-loaded chunk columns
16. create app timing state
17. if no created world was opened, switch into the startup world-select screen before gameplay can run
18. sync the renderer environment from the initial world calendar/weather state
19. return `GameApp`

## Output

- initialized `GameApp`

## Invariants

- during bootstrap, the renderer may exist without a live GPU surface backend
- bootstrap fails fast if the default block registry or block texture set cannot be loaded
- UI atlas load is best-effort; renderer can fall back to a white dummy texture if the atlas file is missing
- the default local player is spawned once during bootstrap
- bootstrap does not synchronously load the full created-world spawn neighborhood; frame lifecycle planning streams chunk loads through jobs
- if a created world is available, bootstrap records a pending spawn anchor so frame job results can snap the `2x2x4` body onto a safe loaded surface later

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
- when a created world is found, bootstrap stages the local player at the created-world preview x/z and lets the normal `LoadChunk -> BuildChunkMesh` lifecycle fill the `7x7` interest area asynchronously
- bootstrap and world reload now log created-world manifest bounds, approximate chunk count, selected spawn chunk, and staged spawn anchor so startup stalls can be distinguished from later streaming work
- when no created world is found, bootstrap skips procedural chunk preload entirely, keeps gameplay behind the world-select startup screen, and waits for the user to create or load a created world
- the main binary default keeps latest-world auto-open disabled so existing created worlds appear in the startup menu instead of being loaded synchronously before the window event loop starts
- bootstrap now also seeds the renderer environment from cached world-owned calendar/climate/weather state instead of leaving startup on a permanently fixed renderer preset
- created-world reload queues a focused region-classification resolve after staging the selected spawn anchor so first gameplay frames do not synchronously resolve atlas structure for HUD/environment state
- the current app-owned world-select screen reuses bootstrap-style helpers to queue create-world work and reload the runtime into a newly selected created-world root without changing lower-layer ownership
- after bootstrap or created-world reload, minimap jobs are seeded only from already-loaded columns; newly streamed chunk results refresh their own minimap columns as they enter `WorldCore`
