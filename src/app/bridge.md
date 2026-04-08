# bridge

## 역할

- 모듈 간 표현 차이를 연결하는 번역 계층
- cross-module DTO mapping은 app 내부에서만 담당한다

## 책임

- platform raw state -> `EcsInputSnapshot`
- ECS gameplay state -> renderer render-ready DTO
- future jobs/world 결과 -> renderer upload request

## 비책임

- gameplay state transition 생성
- world source of truth 수정
- renderer draw 구현
- network protocol 구현

## 입력

- `Platform` state
- `EcsRuntime` state
- future `JobResult`, world dirty info

## 출력

- `EcsInputSnapshot`
- `AppRenderFrameData`
- future `RenderUploadRequest`

## 상태 전이 규칙

- bridge는 가능한 한 stateless translator로 유지한다.
- gameplay 해석은 ECS가 담당한다.
- renderer는 ECS/world 내부 타입을 직접 알지 않는다.

## 불변식

- `platform -> ecs` 경계에서는 raw state를 frame 입력 DTO로만 바꾼다.
- `ecs -> renderer` 경계에서는 render-ready DTO만 만든다.
- renderer는 local player entity나 ECS `Transform`을 직접 query하지 않는다.

## 관련 모듈

- `frame.rs`
- `platform`
- `ecs`
- `renderer`

## 메모

- 현재 최소 구현에는 두 경로가 있다.
  - `platform -> EcsInputSnapshot`
  - `ecs -> AppRenderFrameData`
- `AppRenderFrameData`는 현재 `camera`, `visible_chunks`, `cube_instances`를 가진다.
- local player body-center transform은 플레이어 큐브 한 개의 `RenderCubeInstance`로 번역된다.
- 현재 prototype은 플레이어 큐브를 디버그 가시성 우선으로 조금 크게 그리고, camera distance도 가깝게 둔다.
