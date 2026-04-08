## app

### 역할

- 전체 프로그램의 조립과 프레임 루프 오케스트레이션

### 책임

- program bootstrap
- module init/injection
- main loop posession
- frame sequence definition
- platform → ecs → jobs → renderer flow connection
- exit condition processing

### 비책임

- algorithm implementation
- event parsing implementation

### 데이터

- Platform
- Renderer
- WorldCore
- SimulationCore
- EcsRuntime
- JobSystem
- frame timing
- fixed timestep accumulator
- app config
- simulation config

### 유스케이스

- 프로그램 시작
    - 설정 로드
    - platform 초기화
    - renderer 초기화
    - world/ecs/jobs 초기화
- 프레임 실행
    - OS/window event poll
    - raw state를 ecs resource로 반영
    - pre/update/post schedule 실행
    - jobs 결과 수거
    - renderer 업로드/렌더
- fixed tick 실행
    - 시뮬레이션 결과 반영
- 종료 처리
    - close request 확인
    - 필요 시 저장 flush
    - 자원 정리

### 인터페이스

일반적으로 **시스템 집합 + 스케줄 + 리소스 초기화 함수**를 제공하기. 시스템들은 일반 rust 함수로 정의하고 Schedule::add_systems(…)로 등록하는 형태. Schedule은 시스템과 실행 메타데이터를 담고, run(&but world)로 실행된다

```rust
GameApp::new(config: AppConfig) -> GameApp
GameApp::run(self)

//내부적으로는:
fn begin_frame(&mut self)
fn run_fixed_update(&mut self)
fn run_pre_update(&mut self)
fn run_update(&mut self)
fn run_post_update(&mut self)
fn render(&mut self)
```

### 의존성

- 다른 모든 상위 모듈들 (조립해야하기 때문)

### 불변식

1. app은 각 모듈 간 흐름만 조율하고, 도메인 규칙은 하위 모듈에 위임한다.
2. 프레임 순서는 항상 동일해야 한다.
    - platform poll
    - ecs pre/update/post
    - renderer upload/render
3. frame update와 fixed update 순서는 명확히 분리된다.
4. fixed tick catch-up 정책은 항상 동일하다.
5. app만이 모듈 간 구체 연결 방식을 안다. platform은 ecs를 모르고, world는 renderer를 모른다.
6. 플랫폼 차이로 인해 바뀌는 bootstrap/loop 코드는 가능한 app과 platform에 국한한다.