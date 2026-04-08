# state

## 역할

- app가 소유하는 상위 런타임 상태를 정의한다.
- 모듈 인스턴스와 프레임 타이밍 상태를 한 곳에 모은다.

## 소유 데이터

### GameApp
- `config`
- `platform`
- `ecs`
- `renderer`
- `timing`

### AppTimingState
- `frame_index`
- `frame_dt`
- `last_frame_instant`
- `next_frame_deadline`

## 입력

- bootstrap에서 생성한 module instance
- 매 프레임의 현재 시각
- config의 frame timing policy

## 출력

- runner / frame / fixed가 공통으로 참조하는 app-owned state

## 상태 전이 규칙

- bootstrap 이후 module instance는 `GameApp`이 소유한다.
- 프레임이 실제로 실행될 때만 `frame_index`와 `frame_dt`를 갱신한다.
- `next_frame_deadline`은 frame cap이 있을 때 다음 프레임 시점을 나타낸다.

## 불변식

- frame timing state는 app만이 갱신한다.
- `frame_dt`는 실제 프레임 실행 간격이다.
- 현재 `frame_dt`는 fixed tick이 아니라 frame cadence를 의미한다.

## 비책임

- 개별 모듈의 내부 로직
- raw input 저장
- world source of truth 저장

## 관련 모듈

- `bootstrap.rs`
- `runner.rs`
- `frame.rs`

## 메모

- 현재 최소 구현은 `Platform`, `EcsRuntime`, `Renderer`, `AppConfig`, `AppTimingState`를 실소유한다.
