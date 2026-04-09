# frame

## 역할

- app frame update pipeline을 정의하고 실행한다.

## 책임

- app timing state를 ECS frame delta resource로 주입
- platform snapshot을 ECS 입력으로 bridge
- ECS pre/update/post 실행
- jobs 완료 결과 수거와 world/renderer 반영
- ECS chunk interest를 바탕으로 jobs 요청 제출
- world와 viewport 기준 selection update 호출
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
- current world/job 결과
- current window viewport

## 출력

- 갱신된 ECS world/resource state
- 갱신된 world chunk state
- 갱신된 renderer chunk mesh cache
- 갱신된 `SelectionState`
- discrete gameplay command 로그
- renderer frame render 시도

## 처리 흐름

1. app timing state를 ECS frame delta resource로 주입한다
2. `bridge_platform_to_ecs()`
3. `ecs.run_pre_update()`
4. `ecs.run_update()`
5. `ecs.run_post_update()`
6. 이전에 완료된 jobs 결과를 수거해서 ECS/world/renderer에 반영한다
7. ECS chunk meta를 바탕으로 다음 jobs 요청을 만든다
8. jobs에 요청을 제출한다
9. 다시 완료된 jobs 결과를 수거해 같은 frame에 반영 가능한 범위까지 반영한다
10. 현재 world state와 window viewport를 기준으로 `ecs.update_selection_from_world(...)`를 호출한다
11. discrete command를 drain해서 필요 시 로그 확인한다
12. `bridge_ecs_to_render_frame()` 결과를 `renderer.render(...)`에 전달한다

## 불변식

- frame phase는 `pre -> update -> post` 순서를 유지한다.
- continuous movement는 더 이상 `PlayerCommand`로 로그하지 않는다.
- renderer는 `RenderCameraState`와 `RenderCubeInstance` 같은 render-ready DTO만 받는다.
- selection update는 world/job 결과 반영 이후에 실행되어야 한다.

## 관련 모듈

- `bridge.rs`
- `runner.rs`
- `ecs`
- `world`
- `renderer`

## 메모

- 현재 최소 구현은 플레이어가 위치한 청크 하나를 interest 대상으로 삼아 `GenerateChunk -> BuildChunkMesh -> RenderUploadRequest` 경로를 순차적으로 연결한다.
- render bridge는 quarter-view camera, render-ready visible chunk coord 목록, local player 큐브, 얇은 ground shadow slab, 그리고 hovered face 위의 노란 highlight slab을 렌더 입력으로 만든다.
