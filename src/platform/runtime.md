# runtime

## 역할

- winit adapter
- window creation
- OS/winit event collection
- `PlatformEvent` 생성
- window/input/lifecycle reducer fan-out

## 책임

- event loop / window 초기화
- platform runtime context 보유
- OS 이벤트를 `PlatformEvent`로 정규화
- reducer dispatch 실행
- renderer bootstrap용 `Arc<Window>` 보관과 노출

## 비책임

- 전체 게임 루프 orchestration
- fixed timestep 관리
- ecs 실행
- renderer draw 호출 정책
- gameplay input 해석
- world 수정

## 소유 데이터

- `Arc<Window>`
- event loop runtime context
- normalized event dispatch flow

## 처리 흐름

1. runtime이 OS/winit event를 수신한다.
2. event를 `PlatformEvent`로 정규화한다.
3. 필요하면 하나의 OS event를 여러 `PlatformEvent`로 fan-out한다.
4. window/input/lifecycle reducer에 순서대로 전달한다.
5. 상위 계층이 읽을 수 있는 snapshot state를 유지한다.

## 공개 인터페이스

```rust
PlatformRuntime::new() -> PlatformRuntime
PlatformRuntime::resumed(...)
PlatformRuntime::suspended(...)
PlatformRuntime::handle_window_event(...)
PlatformRuntime::request_redraw(&self)
PlatformRuntime::window_handle(&self) -> Option<Arc<Window>>
```

## 의존성

- winit

## 불변식

- reducer는 winit 타입에 직접 의존하지 않는다.
- runtime은 gameplay state를 만들지 않는다.
- live window는 `Arc<Window>`로 유지해 app/renderer가 안전하게 attach할 수 있게 한다.

## 관련 모듈

- `mod.rs`
- `event.rs`
- `window.rs`
- `input.rs`
- `lifecycle.rs`

## 메모

- 현재 구현은 reducer fan-out 직전에 모든 `PlatformEvent`를 콘솔에 로그로 출력한다.
