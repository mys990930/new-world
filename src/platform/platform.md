## platform

### 역할

- 운영체제/윈도우 시스템과 게임 코어 사이의 경계층
- 운영체제가 주는 신호를 받아서, 게임 쪽이 읽을 수 있는 상태로 바꿔주는 모듈

### 책임

- OS/window event collection
- raw input collection
- window state tracking
- app lifecycle status tracking
- platform-oriented init/exit process
- platform event → normalized platform event/state

### 비책임

- player movement
- block interaction interpretation
- world edit
- chunk load judgment
- render logic process
- ecs system execution
- game rule judgment

### 데이터

- WindowState (width, height, scale_factor, focused, minimized, resized_this_frame, close_requested)
- RawInputState(mouse_screen_pos, mouse_delta, wheel_delta, left/right pressed 상태, key pressed states, modifiers, text input buffer)
- PlatformLifecycleState(app active/inactive, suspended/resumed, quit requested)
- PlatformContext(window handle, event loop 관련 상태, platform-specific runtime context)

### 유스케이스

- 프레임 시작 시 이벤트 수집
    - OS에서 들어온 window/input 이벤트를 poll
    - 내부 raw state 갱신
    - 이번 프레임에 눌렸는가 같은 transient state 계산
- 창 크기 변경 반영
    - resize 이벤트 수신
    - WindowState 갱신
    - 렌더러가 참고할 수 있게 상태 노출
- 마우스/키보드 상태 갱신
    - pressed / just_pressed / just_released 갱신
    - 마우스 좌표/델타 갱신
- 종료 요청 처리
    - 창 닫기 요청 감지 → close_requested event 발행
- 포커스 변화 처리
    - 입력 상태 초기화

### 인터페이스

```rust
Platform::new(...) -> Platform
Platform::resumed(&mut self, event_loop: &ActiveEventLoop)
Platform::suspended(&mut self)
Platform::begin_frame(&mut self)
Platform::handle_window_event(&mut self, window_id: WindowId, event: &WindowEvent) -> bool
Platform::end_frame(&mut self)

Platform::window_state(&self) -> &WindowState
Platform::raw_input_state(&self) -> &RawInputState
Platform::lifecycle_state(&self) -> &PlatformLifecycleState
```

### 의존성

- winit
- ?platform-specific bindings

NOT:

- world, ecs, renderer, game domain… etc.
- app이 platform과 ecs를 연결하고, platform이 ecs를 아는 구조도 피하기

### 불변식

1. platform은 게임 의미를 만들지 않는다 (raw event/state 까지만 다룸)
2. just_pressed, just_released, mouse_delta, wheel_delta 같은 프레임성 상태는 프레임 단위로 초기화/갱신된다
3. 포커스를 잃었을 때 stuck input이 생기지 않도록, 필요한 pressed 상태는 정리 가능해야 한다
4. platform 은 절대 게임 내부에 관여하거나 수정하지 않는다
5. 플랫폼 별 차이 (데스크탑/모바일 등)는 platform 내부에 캡슐화되고, 상위 계층은 가능한 공통 API만 본다.

### 하위 모듈 목록 및 역할

- mod.rs: public facade, re-export, state snapshot getter
- event.rs: normalized platform event enum + shared payload types
- window.rs: WindowState + reducer
- input.rs: RawInputState + reducer
- lifecycle.rs: LifecycleState + reducer
- runtime.rs: winit adapter, window creation, event normalization, reducer dispatch
