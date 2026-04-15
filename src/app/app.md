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
- `Option<CreatedWorldSource>`
- `JobSystem`
- `Renderer`
- `AppUiState`
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
fn should_run_frame(&self, now: Instant) -> bool
fn frame_deadline(&self) -> Option<Instant>
```

### Dependencies

- `platform`
- `ecs`
- `world`
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
- fixed.rs: future fixed timestep orchestration
- bridge.rs: cross-module DTO translation
- ui.rs: app-mode and lightweight overlay state
- shutdown.rs: future teardown / flush

### Current Implementation Notes

- bootstrap still creates the renderer before the real OS window exists, so it starts from `StubSurfaceTarget`
- the real GPU surface still attaches in `runner.rs` during `resumed()`
- the current frame path supports both created-world chunk loading and procedural generation, then meshing and renderer upload
- the current world-aware player slice keeps collision against `WorldCore` outside the pure ECS schedules so world source-of-truth ownership stays in `world`
- the current app-owned screen slice can create and reload created worlds, render a sprite-based world-select layout without stepping gameplay, and add HUD frames without giving renderer any ECS/world dependency
