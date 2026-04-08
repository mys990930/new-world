## app

### 역할

- 전체 프로그램 조립과 프레임 루프 오케스트레이션
- `platform -> app -> ecs -> (simulation) -> world/jobs -> renderer` 흐름의 상위 owner

### 책임

- 프로그램 bootstrap
- 모듈 초기화 / 주입
- 프레임 루프 ownership
- frame update / fixed update 경계 정의
- 프레임 타이밍 정책과 최대 프레임 제한 관리
- 종료 조건 처리

### 비책임

- raw input 파싱 구현
- gameplay 규칙 계산
- world 원본 데이터 수정
- renderer 내부 draw 로직

### 소유 데이터

- `Platform`
- `EcsRuntime`
- `AppConfig`
- `AppTimingState`

### 유스케이스

- 프로그램 시작
  - config 로드
  - platform 초기화
  - ecs 초기화
- 프레임 실행
  - 누적된 platform snapshot을 읽음
  - ECS pre/update/post 실행
  - discrete command 로그 확인
  - redraw 요청
- 프레임 속도 제어
  - `AppConfig::timing.target_frame_rate` 기준으로 다음 프레임 시점을 예약
- 종료 처리
  - close request / quit request 확인
  - event loop 종료

### 공개 인터페이스

```rust
GameApp::new(config: AppConfig) -> GameApp
GameApp::run(self)

fn update(&mut self)
fn bridge_platform_to_ecs(&mut self)
fn begin_timed_frame(&mut self, now: Instant)
fn should_run_frame(&self, now: Instant) -> bool
fn frame_deadline(&self) -> Option<Instant>
```

### 의존성

- 상위 조립 계층이므로 `platform`, `ecs`에 의존

### 불변식

1. app만이 모듈 간 실제 연결을 안다.
2. frame update와 fixed update는 개념적으로 분리된다.
3. 현재 frame loop는 fixed tick이 아니라 app-owned frame cadence다.
4. 최대 프레임 제한은 app timing policy가 담당한다.
5. platform transient state는 프레임이 끝난 뒤 다음 accumulation 구간을 시작할 때만 초기화한다.

### 하위 모듈 목록 및 역할
- mod.rs: public facade, re-export
- config.rs: `AppConfig` / `TimingConfig` 정의
- state.rs: `GameApp`, `AppTimingState` 등 app-owned 상위 상태 정의
- bootstrap.rs: module 생성과 초기 주입
- runner.rs: winit `ApplicationHandler`, frame cadence 제어, 종료 처리
- frame.rs: frame update pipeline orchestration
- fixed.rs: 향후 fixed timestep accumulator와 fixed tick orchestration
- bridge.rs: platform snapshot -> ECS resource 변환
- shutdown.rs: 향후 flush / drain / teardown 처리

### 현재 구현 메모

- 현재 최소 구현은 `platform + ecs`만 실제로 연결되어 있다.
- frame loop는 기본값으로 `60 FPS`를 목표로 제한한다.
- fixed update는 아직 미연결 상태다.
