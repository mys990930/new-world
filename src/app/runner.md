# runner

## 역할

- winit `ApplicationHandler`를 통해 메인 루프를 소유한다.
- frame cadence와 종료 시점을 조율한다.

## 책임

- event loop 진입
- 종료 조건 검사
- frame cap 적용
- 프레임 실행 시점 결정
- redraw 요청

## 비책임

- raw input 파싱
- ECS 시스템 구현
- simulation fixed tick 구현
- renderer draw 구현

## 소유 데이터

- event loop control flow 정책
- frame deadline 판단 로직

## 처리 흐름

1. OS/window 이벤트를 platform에 전달한다.
2. `about_to_wait`에서 종료 여부를 검사한다.
3. frame deadline 이전이면 `WaitUntil(deadline)`로 대기한다.
4. frame deadline에 도달하면 app frame을 실행한다.
5. redraw를 요청한다.
6. 현재 프레임을 끝내고 다음 프레임 accumulation을 위해 platform transient state를 초기화한다.

## 출력

- frame update 실행
- event loop 종료

## 상태 전이 규칙

- 현재 frame cadence는 fixed tick이 아니라 `about_to_wait` 기반 frame loop다.
- frame cap이 켜져 있으면 `ControlFlow::WaitUntil`을 사용한다.
- frame cap이 없으면 `ControlFlow::Poll`을 사용할 수 있다.

## 불변식

- platform input transient는 event loop iteration마다 지우지 않는다.
- transient state는 실제 프레임을 소비한 뒤에만 다음 accumulation을 위해 초기화한다.
- 따라서 입력 이벤트가 frame deadline 전에 도착해도 다음 프레임에서 잃지 않는다.

## 관련 모듈

- `state.rs`
- `frame.rs`
- `platform`

## 메모

- 이전 최소 구현은 사실상 uncapped frame loop였다.
- 현재 기본 정책은 `60 FPS` cap이다.
