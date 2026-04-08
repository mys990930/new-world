# bridge

## 역할

- 모듈 간 표현 차이를 연결하는 번역 계층
- app 내부에서만 쓰이는 cross-module mapping을 담당한다

## 책임

- platform raw state -> ECS input snapshot 변환
- ECS gameplay state -> renderer render-ready DTO 변환
- jobs/world 결과 -> renderer upload request 변환을 위한 경계 제공

## 비책임

- gameplay 상태 전이 생성
- world source of truth 수정
- renderer 내부 draw 구현
- network protocol 구현

## 입력

- `Platform` 상태
- `EcsRuntime` 상태
- 향후 `JobResult`, world dirty info

## 출력

- `EcsInputSnapshot`
- `AppRenderFrameData`
- 향후 `RenderUploadRequest`

## 상태 전이 규칙

- bridge는 가능한 한 stateless translator로 유지한다
- gameplay 의미 해석은 ECS가 담당한다
- renderer는 render-ready DTO만 받고 ECS/world 내부 타입은 직접 모른다

## 불변식

- platform raw input을 gameplay command로 최종 해석하는 책임은 ECS에 있다
- renderer 경계에서는 `RenderCameraState`, visible set 같은 render DTO만 노출한다
- bridge는 소유권 경계를 깨지 않는다

## 관련 모듈

- `frame.rs`
- `platform`
- `ecs`
- `renderer`

## 메모

- 현재 최소 구현에는 두 개의 경로가 있다
  - `platform -> EcsInputSnapshot`
  - `ecs -> AppRenderFrameData`
- 현재 `ecs -> render` 경로는 local player 몸 중심 좌표와 `CameraState.quarter_turns`를 사용해 fixed quarter-view orbit camera를 만든다
- 현재 visible chunk 목록은 아직 빈 리스트다
