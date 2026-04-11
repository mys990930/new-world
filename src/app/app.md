## app

### Role

- top-level runtime orchestration and frame loop ownership
- owner of the `platform -> app -> ecs -> world/jobs -> renderer` flow

### Responsibilities

- bootstrap
- module creation and injection
- frame cadence and redraw policy
- `platform -> ecs` and `ecs/world -> renderer` bridge calls
- baked-world runtime selection ownership
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
- `Option<BakedWorldSource>`
- `JobSystem`
- `Renderer`
- `AppConfig`
- `AppTimingState`

### Public Interface
```rust
GameApp::new(config: AppConfig) -> GameApp
GameApp::run(self)

fn update(&mut self)
fn render(&mut self)
fn bridge_platform_to_ecs(&mut self)
fn bridge_ecs_to_render_frame(&self) -> AppRenderFrameData
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
4. baked-world runtime selection is app-owned because it decides whether chunk acquisition should load from disk or fall back to generation

### Submodules

- mod.rs: public facade, re-export
- config.rs: `AppConfig` / `TimingConfig`
- state.rs: `GameApp`, `AppTimingState`
- bootstrap.rs: module creation, baked-world detection, initial preload, and spawn placement
- runner.rs: winit `ApplicationHandler`, frame cadence, redraw, shutdown handling
- frame.rs: frame update pipeline orchestration
- fixed.rs: future fixed timestep orchestration
- bridge.rs: cross-module DTO translation
- shutdown.rs: future teardown / flush

### Current Implementation Notes

- bootstrap still creates the renderer before the real OS window exists, so it starts from `StubSurfaceTarget`
- the real GPU surface still attaches in `runner.rs` during `resumed()`
- the current frame path supports both baked chunk loading and procedural generation, then meshing and renderer upload
- the current world-aware player slice keeps collision against `WorldCore` outside the pure ECS schedules so world source-of-truth ownership stays in `world`
