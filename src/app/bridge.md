# bridge

## 역할

- 모듈 간 표현 차이를 연결하는 번역 계층
- app 내부에서만 허용되는 cross-module mapping을 담당한다

## 책임

- platform raw state → ecs resource 변환
- jobs completion → ecs/world 반영용 형태 변환
- world/ecs dirty state → renderer upload request 변환
- 필요 시 simulation/world 결과 → jobs 후속 요청 변환

## 비책임

- gameplay 의미 생성
- world source of truth 수정 로직 구현
- renderer 업로드 구현
- network protocol 구현

## 입력

- Platform 상태
- JobResult
- ECS output / command / meta state
- World dirty info
- Renderer upload input

## 출력

- EcsInputResource
- JobApplyCommand
- RenderUploadRequest
- optional save/remesh request

## 상태 전이 규칙

- bridge는 가능한 한 stateless하게 유지한다
- 변환 규칙은 deterministic해야 한다
- 한 모듈의 내부 타입이 다른 모듈 public API로 새어나가지 않도록 조정한다

## 불변식

- bridge는 번역만 하고 정책 판단은 하지 않는다
- platform raw input을 gameplay command로 최종 해석하는 책임은 ecs에 있다
- world와 renderer의 소유 경계를 bridge가 깨면 안 된다

## 관련 모듈

- frame.rs
- fixed.rs
- platform
- ecs
- jobs
- renderer
- world

## 메모

- 이 파일은 app 내부에서 제일 더러워지기 쉬우니, 변환 방향별로 하위 파일 분리도 고려할 수 있다
- 예: bridge/input.rs, bridge/jobs.rs, bridge/render.rs
- 현재 최소 구현은 platform snapshot을 `EcsInputSnapshot`으로 변환해 ecs resource로 주입하는 경로만 포함한다
