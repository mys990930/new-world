## platform

### 역할

- 시간 기반 월드 규칙의 실행 코어
- 생태계, 전기, 유체, 화재, 작물 성장 등 fixed tick 기반 시뮬레이션 규칙 담당
- world를 직접 소유하지 않고, world에 적용할 결과를 계산함

### 책임

- fixed tick 단위 시뮬레이션 step 수행
- 시뮬레이션 규칙별 서브시스템 실행
    - ecology tick
    - power tick
    - fluid tick
    - fire tick
    - farming tick
- world snapshot / query input을 기반으로 결과 계산
- WorldEdit, SimEvent, DirtyChunkHint 등의 결과 생성
- deterministic execution 보장
- 필요 시 region 단위 / subsystem 단위 step 실행

### 비책임

- world source of truth 소유
- raw input 처리
- game command 해석
- fixed tick 스케줄링 자체
- worker thread/task 실행 자체
- draw call / gpu upload
- platform event 처리

### 데이터

#### Config / Runtime Data

- SimulationConfig (subsystem enabled flags, tick rates, max steps per frame)
- FixedStepConfig (target dt, max catch-up steps)
- SimTick
- SubSystemId (Ecology, Power, Fluid, Fire, Farming)

#### Input Data
- SimRegion (active sim chunk set, optional priority/radius)
- SimInput (tick, subsystem, region, world snapshot/query accessor, optional environment context)

#### Output Data
- SimulationResult(world_edits, events, dirty_chunks, followup_requests)
- SimEvent(TreeGrown, PowerStateChanged, WaterSpread, FireSpread, CropGrown)
- SimFollowupRequest(SaveHint, RemeshHint, ReplicationHint)

### 유스케이스

- 전기 틱 실행
    - 활성 청크 내 power graph / signal propagation 계산
    - 블록 상태 변경 결과 반환
- 생태계 틱 실행
    - 식생 성장 / 번식 / 자연 상태 변화 계산
    - world edit와 sim event 반환
- 유체 틱 실행
    - 물/용암/기타 유체 전파 계산
    - 변경된 블록 상태 반환
- 화재 틱 실행
    - 인접 flammable block 검사
    - 연소 / 확산 결과 반환
- 작물 성장 틱 실행
    - 환경 조건 기반 성장 단계 갱신
    - dirty chunk / save hint 반환
- 대규모 영역 시뮬레이션
    - region 단위로 입력을 받아 step 실행
    - jobs를 통해 비동기 실행 가능

### 인터페이스

일반적으로 고수준 시뮬레이션 실행 API + subsystem별 step 함수를 제공하기.

```rust
SimulationCore::new(config: SimulationConfig) -> SimulationCore

SimulationCore::step(
    subsystem: SubsystemId,
    input: SimInput,
) -> SimulationResult

SimulationCore::step_all(
    tick: SimTick,
    region: SimRegion,
    input: SimInputBundle,
) -> Vec<SimulationResult>
```

또는 subsystem 별로:

```rust
EcologySim::step(input: SimInput) -> SimulationResult
PowerSim::step(input: SimInput) -> SimulationResult
FluidSim::step(input: SimInput) -> SimulationResult
FireSim::step(input: SimInput) -> SimulationResult
FarmingSim::step(input: SimInput) -> SimulationResult
```

### 의존성

- world
- block registry/biome config/sim config

NOT:

- platform/renderer/app
- ecs, jobs 내부 구현

### 불변식

1. simulation은 world source of truth를 직접 소유하지 않는다.
2. simulation은 world를 직접 mutate하지 않고, 반드시 WorldEdit / SimulationResult를 통해 결과를 반환한다.
3. 같은 입력에 대해 같은 결과가 나오는 deterministic step을 유지한다.
4. fixed tick 규칙은 frame rate와 분리되어야 한다.
5. subsystem 간 실행 순서는 명시적이어야 한다.
6. simulation 결과는 영향 청크를 정확히 반환해야 한다.
7. 시뮬레이션 규칙은 raw input이나 platform state에 직접 의존하지 않는다.
8. 무거운 simulation step은 jobs로 위임할 수 있지만, 규칙의 의미와 결과 형식은 simulation이 정의한다.

### 하위 모듈 목록 및 역할
