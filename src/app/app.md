## app

### 역할

- 프로그램 전체 조립과 frame loop orchestration
- `platform -> app -> ecs -> (simulation) -> world/jobs -> renderer` 흐름의 상위 owner

### 책임

- bootstrap
- 모듈 생성과 주입
- frame cadence와 redraw 타이밍 제어
- platform snapshot을 ECS/renderer bridge로 연결
- 종료 조건 처리

### 비책임

- raw input 수집
- gameplay rule 계산
- world source of truth 수정
- GPU draw 구현

### 소유 데이터

- `Platform`
- `EcsRuntime`
- `WorldCore`
- `JobSystem`
- `Renderer`
- `AppConfig`
- `AppTimingState`

### 공개 인터페이스

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

### 의존성

- `platform`
- `ecs`
- `world`
- `jobs`
- `renderer`

### 불변식

1. app만이 모듈 간 실제 연결을 소유한다.
2. frame cadence는 fixed tick이 아니라 app-owned frame policy다.
3. renderer는 app bridge가 만든 render-ready DTO만 받는다.

### 하위 모듈 목록 및 역할

- mod.rs: public facade, re-export
- config.rs: `AppConfig` / `TimingConfig`
- state.rs: `GameApp`, `AppTimingState`
- bootstrap.rs: module 생성과 초기 주입
- runner.rs: winit `ApplicationHandler`, frame cadence, redraw, 종료 처리
- frame.rs: frame update pipeline orchestration
- fixed.rs: future fixed timestep orchestration
- bridge.rs: cross-module DTO translation
- shutdown.rs: future teardown / flush

### 현재 구현 메모

- 현재 bootstrap은 window가 아직 없으므로 `StubSurfaceTarget`으로 renderer를 먼저 만든다.
- 실제 GPU surface attach는 `runner.rs`의 `resumed()`에서 window 생성 직후 수행한다.
- 현재 frame path는 `ecs -> jobs -> world -> renderer upload -> renderer draw`의 최소 chunk plane vertical slice까지 연결돼 있다.
- app frame은 world/job 결과 반영 뒤 `cursor -> world raycast -> SelectionState` 경로를 갱신한다.
- render path는 ECS camera state, local player body-center transform, selection state를 render DTO로 바꿔, 생성된 chunk plane 위의 플레이어 큐브와 ground shadow slab, 그리고 hovered face 위 노란 highlight slab을 함께 그린다.
