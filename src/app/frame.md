# frame

## 역할

- app frame update pipeline을 정의하고 실행한다.

## 책임

- platform snapshot을 ECS 입력으로 bridge
- ECS pre/update/post 실행
- discrete command 로그 확인
- render bridge 결과로 renderer 호출

## 비책임

- frame cap 계산
- fixed timestep accumulator
- simulation 실행
- renderer 내부 draw 구현

## 입력

- 현재 platform snapshot
- 현재 frame 시점의 app timing state

## 출력

- 갱신된 ECS world/resource state
- discrete gameplay command 로그
- renderer frame render 시도

## 처리 흐름

1. `bridge_platform_to_ecs()`
2. `ecs.run_pre_update()`
3. `ecs.run_update()`
4. `ecs.run_post_update()`
5. discrete command를 drain해서 필요 시 로그 확인
6. `bridge_ecs_to_render_frame()` 결과를 `renderer.render(...)`에 전달

## 불변식

- frame phase는 `pre -> update -> post` 순서를 유지한다.
- continuous movement는 더 이상 `PlayerCommand`로 로그하지 않는다.
- renderer는 `RenderCameraState`와 `RenderCubeInstance` 같은 render-ready DTO만 받는다.

## 관련 모듈

- `bridge.rs`
- `runner.rs`
- `ecs`
- `renderer`

## 메모

- 현재 render bridge는 quarter-view camera와 local player 큐브 한 개를 렌더 입력으로 만든다.
- visible chunk 목록은 아직 비어 있고, chunk draw/upload 경로는 이후 단계에서 확장한다.
