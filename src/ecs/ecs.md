## ecs
### 역할

- 게임 상태 전이의 중심
- 지금 게임이 어떤 상태고, 이번 프레임에 어떻게 바뀌는가
- **처음부터 멀티플레이를 고려해서 구조를 짤 것**
    - input → command 생성 → command 소비 (apply) 구조로 반드시 짜자.
    - 잘못된 구조: 좌클릭 들어오면 바로 world.set_block()
    - 좋은 구조: 좌클릭 시 ecs가 BlockBreakCommand 생성 후, 싱글이라면 바로 적용해도 되지만 멀티라면 그 command를 서버로 전송

### 책임

- game status save
- frame/tick level status transition
- input interpretation (at a game’s viewpoint)
- player/npc/item… dynamic entity management
- chunk meta status management
- event/command handling
- *jobs* result handling →  add to game status
- system execution order definition
- active simulation region calculation
- per subsystem sim request init
- sim result conversion to following world/jobs/renderer jobs

### 비책임

- chunk direct i/o
- procedural world generation algorithm
- meshing algorithm
- main loop bootstraping (app에서 할 것)

### 데이터

#### Resource

- EcsInputSnapshot
- PlayerCommandBuffer
- WorldTime
- SimClock
- ActiveSimRegion
- PendingSimulationResults
- SimulationControlState
- WeatherState
- ChunkStates (visible, loading, meshing, dirty_mesh, save_pending)
- PendingJobResults
- CameraState
- SelectionState
- DebugFlags

#### Entity/Component

- Player
- Transform
- Velocity
- MoveTarget
- Inventory
- Health
- AnimalAI
- DroppedItem
- InteractionTarget

#### Event/Command

- MoveScreenCommand
- PrimaryActionCommand / BlockPlaceRequest
- RotateCameraRequest
- CameraRecenterRequest
- ChunkLoadRequested
- ChunkMeshRequested

### 유스케이스

- raw input을 게임 의미로 변환
    - WASD 화면 기준 이동 → 이동 명령
    - 좌클릭 → 상호작용/파괴 요청
    - 우클릭 → 블록 배치 요청
    - Q/E → 카메라 90도 회전 요청
    - Y → 카메라 리센터 요청
- 플레이어/엔티티 상태 갱신
    - 이동
    - 속도/행동 갱신
    - AI 상태 전이
- 월드 상호작용 요청 처리
    - 블록 파괴 요청 생성/소비
    - world.apply_edit(…) 호출
    - dirty 청크 표시
- visible chunk 계산
    - 플레이어 위치 기반 필요 청크 산출
        - or 카메라 위치 기반 필요 청크 산출
- 플레이어 주변/관심 범위 기반 시뮬레이션 활성 영역 산출
    - sim result 반영 후 dirty chunk, save reuqest, remesh request 생성
- jobs 결과 반영
    - 로드 완료 청크를 world에 삽입
    - 메싱 완료 결과를 renderer 업로드 큐로 넘김
- 후속 작업 요청 생성
    - 청크 로드 요청
    - 메싱 요청
    - 저장 요청

### 인터페이스

일반적으로 **시스템 집합 + 스케줄 + 리소스 초기화 함수**를 제공하기. 시스템들은 일반 rust 함수로 정의하고 Schedule::add_systems(…)로 등록하는 형태. Schedule은 시스템과 실행 메타데이터를 담고, run(&but world)로 실행된다

```rust
EcsRuntime::new() -> EcsRuntime
EcsRuntime::insert_resource<T>(&mut self, value: T)
EcsRuntime::world(&self) -> &World
EcsRuntime::world_mut(&mut self) -> &mut World

EcsRuntime::run_pre_update()
EcsRuntime::run_update()
EcsRuntime::run_post_update()
EcsRuntime::run_fixed_update()
```

### 의존성

- bevy_ecs
- world
- simulation
- jobs의 request/result 타입

NOT:

- platform 내부 구현
    - raw state를 값으로 받되, platform::Window같은 구체 구현 타입은 받지 않기
- renderer의 gpu 세부 구현
- app

### 불변식

1. ecs는 게임 의미를 다룬다. raw OS 이벤트는 직접 다루지 않는다.
2. 개별 블록 원본 데이터는 world가 SoT, ecs는 청크 메타 상태만 가진다.
3. 한 프레임 안에서 입력 해석 → 상태 갱신 → 후속 작업 요청 순서는 일관되어야 한다.
4. fixed tick에서만 적용되는 상태 전이는 frame update와 섞이지 않는다.
5. simulation 결과 반영 순서는 명확해야 한다.
6. jobs 결과 반영 순서는 명확해야 한다.

### 현재 구현 메모

- 현재 최소 구현은 `EcsInputSnapshot -> PlayerCommandBuffer` 변환까지만 제공한다
- 이동 기준은 화면 기준이며, `Q/E`는 90도 회전, `Y`는 카메라 리센터로 해석한다
