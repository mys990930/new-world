# runtime

## 역할

- winit adapter
- window creation
- OS/winit 이벤트 수집
- PlatformEvent 생성
- 각 state reducer 호출

## 책임

- event loop / window 초기화
- platform-specific runtime context 보유
- 외부 이벤트를 PlatformEvent로 정규화
- window / input / lifecycle 상태 갱신 흐름 실행

## 비책임

- 전체 게임 루프 orchestration
- fixed timestep 관리
- ecs 실행
- renderer draw 호출 정책
- gameplay input 해석
- world 수정

## 소유 데이터

- window handle
- event loop 관련 context
- platform runtime context
- 필요 시 event queue 또는 pending event buffer

## 처리 흐름

1. runtime이 OS/winit 이벤트를 수신한다
2. 해당 이벤트를 PlatformEvent로 정규화한다
3. event 종류에 따라 window/input/lifecycle reducer에 전달한다
4. 갱신된 state snapshot을 상위가 읽을 수 있게 유지한다

## 외부 인터페이스

```rust
PlatformRuntime::new(...) -> PlatformRuntime
PlatformRuntime::begin_frame(&mut self)
PlatformRuntime::ingest_os_events(&mut self)
PlatformRuntime::end_frame(&mut self)

PlatformRuntime::window_state(&self) -> &WindowState
PlatformRuntime::input_state(&self) -> &RawInputState
PlatformRuntime::lifecycle_state(&self) -> &LifecycleState
```
## 의존성

- winit
- platform-specific bindings (필요 시)

## 불변식

- winit 세부사항은 runtime 내부에 캡슐화된다
- window/input/lifecycle reducer는 가능하면 winit 타입에 직접 의존하지 않는다
- runtime은 gameplay 의미를 만들지 않는다
- app이 전체 프레임 순서의 주인이다

## 관련 모듈

- mod.rs가 외부에 facade를 제공
- event.rs를 생성/사용
- window.rs / input.rs / lifecycle.rs를 조립
- app이 runtime 상태를 읽어 다음 단계로 넘긴다

## 메모

- winit 버전에 따라 poll 기반보다 callback 기반 설계가 더 자연스러울 수 있다
- 따라서 API 이름은 최종 구현 전에 재검토 가능