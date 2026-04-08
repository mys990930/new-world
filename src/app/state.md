# state

## 역할

- app이 소유하는 상위 런타임 상태를 정의한다
- 모듈 핸들과 타이밍 상태를 한 곳에 모은다

## 소유 데이터

### GameApp
- platform
- renderer
- world
- ecs
- simulation
- jobs
- config
- timing state
- exit flag / shutdown state

### AppTimingState
- last_frame_instant
- frame_dt
- accumulator
- frame_index
- optional stats

### AppExitState
- exit_requested
- exit_reason
- flush_required
- shutdown_started

## 입력

- bootstrap에서 생성된 모듈 인스턴스
- 매 프레임 계산되는 dt
- platform/lifecycle의 종료 신호

## 출력

- runner/frame/fixed/shutdown이 공통으로 사용하는 app-owned state

## 상태 전이 규칙

- bootstrap 이후 모든 상위 모듈 핸들은 GameApp 내부에 저장된다
- frame 시작 시 frame_dt 갱신
- fixed update 실행 시 accumulator 갱신/차감
- 종료 요청 시 AppExitState 갱신

## 불변식

- app만이 platform/world/ecs/jobs/renderer/simulation의 구체 연결을 안다
- world가 renderer를 모르고, platform이 ecs를 모르는 구조를 app state가 깨면 안 된다
- timing state는 app 내부에서만 소유/갱신한다

## 비책임

- 각 모듈 내부 알고리즘
- gameplay 상태 저장
- raw input 저장
- world source of truth 보유

## 관련 모듈

- bootstrap.rs가 GameApp 생성
- runner.rs가 GameApp을 소유하고 루프를 실행
- frame.rs / fixed.rs / shutdown.rs가 GameApp을 갱신

## 메모

- AppContext / GameApp / AppRuntime 중 네이밍 하나로 통일하는 게 좋다