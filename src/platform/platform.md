## platform

### 역할

- OS/window/raw input과 게임 코어 사이의 경계 계층
- 운영체제 신호를 게임이 읽을 수 있는 normalized state로 바꾼다

### 책임

- OS/window event collection
- raw input collection
- window state tracking
- app lifecycle state tracking
- platform-oriented init/exit process
- normalized platform event/state 유지
- renderer bootstrap을 위한 window handle 노출

### 비책임

- player movement
- block interaction interpretation
- world edit
- render logic process
- ecs system execution
- game rule judgment

### 데이터

- `WindowState`
- `RawInputState`
- `LifecycleState`
- `PlatformContext(window handle, event loop runtime context)`

### 공개 인터페이스

```rust
Platform::new(...) -> Platform
Platform::resumed(&mut self, event_loop: &ActiveEventLoop)
Platform::suspended(&mut self)
Platform::begin_frame(&mut self)
Platform::handle_window_event(&mut self, window_id: WindowId, event: &WindowEvent) -> bool
Platform::end_frame(&mut self)

Platform::request_redraw(&self)
Platform::window_handle(&self) -> Option<Arc<Window>>

Platform::window_state(&self) -> &WindowState
Platform::raw_input_state(&self) -> &RawInputState
Platform::lifecycle_state(&self) -> &LifecycleState
```

### 의존성

- winit
- platform-specific bindings

### 불변식

1. platform은 gameplay state를 만들지 않는다.
2. frame-transient input은 frame 단위로만 초기화한다.
3. window handle은 renderer bootstrap 용도로만 노출되고, gameplay 모듈 경계를 넓히지 않는다.

### 하위 모듈 목록 및 역할

- mod.rs: public facade, re-export, state snapshot getter
- event.rs: normalized platform event enum + shared payload types
- window.rs: `WindowState` + reducer
- input.rs: `RawInputState` + reducer
- lifecycle.rs: `LifecycleState` + reducer
- runtime.rs: winit adapter, window creation, event normalization, reducer dispatch
