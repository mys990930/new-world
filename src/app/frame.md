# frame

## 역할

- frame update pipeline을 정의하고 실행한다.

## 책임

- platform snapshot을 ECS 입력으로 bridge
- ECS pre/update/post 실행
- discrete command 로그 확인
- redraw 시점의 renderer 호출

## 비책임

- frame cap 계산
- fixed timestep accumulator
- simulation 실행
- renderer 내부 draw 구현

## 입력

- 누적된 platform snapshot
- 현재 frame 시점의 app timing state

## 출력

- 갱신된 ECS world/resource state
- discrete gameplay command 로그
- renderer render 시도

## 처리 흐름

1. `bridge_platform_to_ecs()`
2. `ecs.run_pre_update()`
3. `ecs.run_update()`
4. `ecs.run_post_update()`
5. 필요 시 discrete command를 drain해서 로그로 확인
6. redraw 시점에는 `render()`가 renderer resize 동기화와 render 호출을 수행

## 불변식

- frame phase는 `pre -> update -> post` 순서를 유지한다.
- continuous movement는 더 이상 `PlayerCommand`로 로그되지 않는다.
- 현재 이동 의도는 ECS 내부 `MoveWorldIntent` resource로 유지된다.
- 현재 render 입력은 app이 임시로 소유하는 default camera와 빈 visible chunk 목록이다.

## 비책임

- fixed tick 반복
- close / flush 처리
- render upload / draw

## 관련 모듈

- `bridge.rs`
- `runner.rs`
- `ecs`
- `renderer`

## 메모

- 현재 최소 구현에서 app 로그는 `PrimaryAction`, `PlaceBlock`, `RotateCamera`, `RecenterCamera` 같은 discrete command만 대상으로 본다.
- renderer 연결은 되었지만, 아직 world/ecs 기반 render DTO bridge는 들어가지 않았다.
