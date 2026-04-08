# runner

## 역할

- winit `ApplicationHandler`를 통해 event loop를 소유하고 frame cadence를 제어한다.

## 책임

- event loop 진입
- 종료 조건 검사
- `resumed()` 시 platform window 생성과 renderer live surface attach
- frame cap 적용
- redraw 요청
- `RedrawRequested`에서 renderer 호출

## 비책임

- raw input 수집
- ECS system 구현
- simulation fixed tick 구현
- GPU draw 구현

## 처리 흐름

1. `resumed()`에서 platform이 window를 만든다.
2. 같은 시점에 `Platform::window_handle()`을 통해 renderer에 live surface를 attach한다.
3. `about_to_wait()`에서 종료 여부와 frame deadline을 검사한다.
4. deadline에 도달하면 app frame을 한 번 실행한다.
5. redraw를 요청한다.
6. `RedrawRequested`에서 `render()`를 호출한다.
7. frame이 끝난 뒤 platform transient state를 다음 frame을 위해 초기화한다.

## 상태 전이 규칙

- frame cadence는 fixed tick이 아니라 `about_to_wait` 기반 frame loop다.
- frame cap이 켜져 있으면 `ControlFlow::WaitUntil(deadline)`을 사용한다.
- frame cap이 없으면 `ControlFlow::Poll`을 사용한다.

## 불변식

- live surface attach는 window가 존재한 뒤에만 수행한다.
- platform transient input은 실제 frame이 끝난 뒤에만 초기화한다.

## 관련 모듈

- `state.rs`
- `frame.rs`
- `platform`
- `renderer`

## 메모

- 기본 정책은 `60 FPS` cap이다.
- renderer는 bootstrap에서 stub로 시작하고, `resumed()`에서 real `wgpu` surface/backend를 붙인다.
