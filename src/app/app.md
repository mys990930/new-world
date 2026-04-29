## app

### Role

- top-level runtime orchestration and frame loop ownership
- owner of the `platform -> app -> ecs -> world/jobs -> renderer` flow

### Responsibilities

- bootstrap
- module creation and injection
- frame cadence and redraw policy
- `platform -> ecs` and `app/ecs/world -> renderer` bridge calls
- created-world runtime selection ownership
- top-level app-mode and overlay ownership
- top-level create-world / created-world selection screen ownership
- top-level minimap cache ownership and jobs-based refresh policy
- top-level chunk unload application ownership
- top-level shutdown handling

### Non-Responsibilities

- raw OS event capture
- gameplay rule evaluation internals
- world source-of-truth mutation internals
- GPU draw implementation details

### Owned Data

- `Platform`
- `EcsRuntime`
- `WorldCore`
- `SimulationCore`
- `Option<CreatedWorldSource>`
- `JobSystem`
- `Renderer`
- `AppUiState`
- `AppMinimapCache`
- pending player spawn anchor
- `AppConfig`
- `AppTimingState`

### Public Interface
```rust
GameApp::new(config: AppConfig) -> GameApp
GameApp::run(self)

fn update(&mut self)
fn render(&mut self)
fn handle_ui_shortcuts(&mut self)
fn gameplay_active(&self) -> bool
fn bridge_platform_to_ecs(&mut self)
fn bridge_app_to_render_frame(&self) -> AppRenderFrameData
fn begin_timed_frame(&mut self, now: Instant)
fn run_fixed_updates(&mut self)
fn should_run_frame(&self, now: Instant) -> bool
fn frame_deadline(&self) -> Option<Instant>
```

### Dependencies

- `platform`
- `ecs`
- `world`
- `simulation`
- `jobs`
- `renderer`

### Invariants

1. app is the only layer that owns concrete module wiring
2. frame cadence is app-owned policy, not a platform-owned policy
3. renderer only receives render-ready DTOs built by app bridge code
4. created-world runtime selection is app-owned because it decides whether chunk acquisition should load from disk or fall back to generation
5. top-level screen mode stays app-owned so non-gameplay screens do not force ECS/world ownership changes
6. top-level menu actions such as create-world / created-world reload stay app-owned so lower layers keep their existing responsibilities

### Submodules

- mod.rs: public facade, re-export
- config.rs: `AppConfig` / `TimingConfig`
- state.rs: `GameApp`, `AppTimingState`
- bootstrap.rs: module creation, created-world detection, initial preload, and spawn placement
- runner.rs: winit `ApplicationHandler`, frame cadence, redraw, shutdown handling
- frame.rs: frame update pipeline orchestration
- fixed.rs: fixed timestep orchestration and world-time-to-render-environment sync
- bridge.rs: shared bridge DTO surface
- bridge_input.rs: platform snapshot to ECS input translation
- bridge_scene.rs: scene-frame and mesh-upload render translation
- bridge_ui.rs: HUD/menu/minimap sprite translation
- minimap.rs: app-owned minimap cache, chunk-column rebuild scheduling state, viewport composition
- ui.rs: app-mode and lightweight overlay state
- shutdown.rs: future teardown / flush

### Current Implementation Notes

- bootstrap still creates the renderer before the real OS window exists, so it starts from `StubSurfaceTarget`
- the real GPU surface still attaches in `runner.rs` during `resumed()`
- the current frame path supports both created-world chunk loading and procedural generation, then meshing and renderer upload
- created-world opening stages the player at the selected spawn x/z and streams the spawn neighborhood through jobs instead of synchronously preloading the full interest area on the main thread
- gameplay frames apply completed job results through a small per-frame budget so chunk load/mesh/minimap bursts do not monopolize input and renderer cadence
- app now also owns the first fixed-tick slice: it accumulates frame time, advances ECS fixed state, calls the simulation time subsystem, applies world-owned calendar/climate/weather updates, and refreshes the renderer environment
- app now also owns steady-state chunk unload application because it is the layer that can coordinate `world.remove_chunk(...)`, renderer mesh removal, minimap cache invalidation, and stale-result acceptance in one place
- the current minimap path is app-owned cached state: chunk load/generate results trigger background minimap-column rebuild jobs, and render bridging only composes the current player-centered viewport from cached column data
- the current world-aware player slice keeps collision against `WorldCore` outside the pure ECS schedules so world source-of-truth ownership stays in `world`
- the current app-owned screen slice can create and reload created worlds, render a mouse-driven sprite-based world-select layout with typed numeric fields, a scrollable created-world list, and create-job loading feedback without stepping gameplay, and add HUD frames without giving renderer any ECS/world dependency
- the main binary no longer hardcodes or auto-opens a created-world root by default; existing created worlds are discovered by the app-owned world-select screen instead of being synchronously loaded before the window event loop starts
