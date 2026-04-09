## ecs

### 역할

- 게임 상태 전이의 중심 계층
- 입력 상태를 gameplay 의미로 해석하고, 후속 world/simulation/jobs 요청의 기반 상태를 만든다
- 멀티플레이를 고려한 command / intent 경계를 유지한다

### 책임

- frame/tick 수준 상태 전이
- input interpretation
- player 중심 entity/component 관리
- camera 상태 관리
- selection 상태 관리
- chunk meta 상태 관리
- jobs 결과 반영
- 시스템 실행 순서 정의

### 비책임

- raw OS event 수집
- world 원본 block 데이터 소유
- chunk direct I/O
- procedural generation
- meshing 알고리즘
- main loop bootstrap

### 데이터

#### Resource

- `EcsInputSnapshot`
- `PlayerCommandBuffer`
- `MoveWorldIntent`
- `FrameDeltaSeconds`
- `PlayerMovementConfig`
- `CameraState`
- `LocalPlayerEntity`
- `ChunkStates`
- `SelectionState`
- `PendingJobResults`
- `ActiveSimRegion`

#### Entity / Component

- `Player`
- `Transform`
- `Velocity`
- `Inventory`
- `Health`
- `InteractionTarget`

#### Event / Command

- `PrimaryAction`
- `PlaceBlock`
- `RotateCamera`
- `RecenterCamera`

### 유스케이스

- raw input을 gameplay 의미로 해석
  - `WASD`는 화면 기준 이동 상태로 입력된다
  - `좌클릭`은 기본 행위 요청
  - `우클릭`은 블록 배치 요청
  - `Q/E`는 카메라 90도 회전 요청
  - `Y`는 카메라 리센터 요청
- 플레이어 이동 의도 생성
  - 화면 기준 입력은 command가 아니라 frame input state로 유지한다
  - `CameraState`의 quarter rotation을 먼저 반영한다
  - 같은 프레임에 회전과 이동이 같이 오면 회전 후 기준으로 `MoveWorldIntent`를 계산한다
  - `MoveWorldIntent`는 world 기준 이동 의미이며 멀티플레이 경계에도 적합하다
- 쿼터뷰 좌표계 해석
  - 창 기준 상하좌우와 월드 기준 동서남북은 일치하지 않는다
  - 기본 쿼터뷰에서 화면 우측 상단이 북쪽, 화면 우측 하단이 동쪽이다
  - 따라서 화면 기준 이동은 world axis로 투영한 뒤 사용한다
- 카메라 상태 갱신
  - 4방향 쿼터뷰 회전
  - `Y`나 좌/우클릭 상호작용 시 1회성 fast recenter boost 신호
  - slow tracking / bias / deadzone은 향후 camera 시스템이 확장한다
- 커서 기반 selection 갱신
  - app가 프레임마다 mouse position과 viewport를 전달한다
  - ECS는 current camera basis와 local player transform을 기준으로 orthographic ray를 만든다
  - world raycast 결과를 `SelectionState`로 저장한다
  - app bridge는 그 상태를 노란 face highlight 렌더 입력으로 바꾼다

### 공개 인터페이스

```rust
EcsRuntime::new() -> EcsRuntime
EcsRuntime::insert_resource<T>(&mut self, value: T)
EcsRuntime::world(&self) -> &World
EcsRuntime::world_mut(&mut self) -> &mut World

EcsRuntime::run_pre_update()
EcsRuntime::run_update()
EcsRuntime::run_post_update()
EcsRuntime::run_fixed_update()

EcsRuntime::spawn_default_player()
EcsRuntime::drain_player_commands() -> Vec<PlayerCommand>
EcsRuntime::move_world_intent() -> MoveWorldIntent
EcsRuntime::update_selection_from_world(
    world: &WorldCore,
    viewport_width: u32,
    viewport_height: u32,
)
EcsRuntime::selection_state() -> SelectionState
```

### 의존성

- `bevy_ecs`
- `world`
- `simulation`
- `jobs`

NOT:

- `platform` 구현
- renderer GPU 구현
- app loop ownership

### 불변식

1. ECS는 raw OS event를 직접 다루지 않는다.
2. world 원본 block 데이터는 ECS가 아니라 world가 소유한다.
3. discrete 행동과 continuous 이동 의도는 같은 표현으로 섞지 않는다.
4. 화면 기준 입력과 world 기준 intent는 별도 경계로 유지한다.
5. 같은 프레임의 회전은 그 프레임 이동 intent 계산에 먼저 반영된다.
6. selection policy는 ECS가 소유하지만 실제 블록 step/raycast는 world query를 사용한다.

### 하위 모듈 목록 및 역할
- mod.rs: public facade, re-export
- runtime.rs: `EcsRuntime`, `World`/`Schedule` 소유, resource 초기화, pre/update/post/fixed 실행 진입점
- input.rs: `EcsInputSnapshot`, frame 입력 resource, discrete command 후보 생성
- command.rs: `PlayerCommand`, `MoveWorldIntent`, ECS 내부 command/request buffer 정의
- player.rs: `Player`/`Transform`/`Velocity`, local player spawn, 화면 기준 이동 상태를 world 기준 이동 intent로 변환
- camera.rs: `CameraState`, 4방향 쿼터뷰 회전 상태, recenter one-shot boost 신호, shared quarter-view basis helper
- selection.rs: world raycast 기반 hover target 상태 정의와 최소 selection update 규칙
- chunk.rs: player 기준 interest / camera 기준 visible chunk meta 상태 정의
- jobs.rs: jobs 결과 반영과 후속 요청 생성 규칙
- fixed.rs: fixed tick용 simulation 흐름 정의

### 현재 구현 메모

- 현재 최소 구현은 `EcsInputSnapshot -> PlayerCommandBuffer + MoveWorldIntent`까지 연결되어 있다.
- discrete command는 app에서 로그로 확인할 수 있다.
- `MoveWorldIntent`는 local player `Velocity`에 반영된다.
- local player `Velocity`는 같은 frame의 `FrameDeltaSeconds`를 사용해 `Transform.translation`에 적분된다.
- 기본 local player는 bootstrap 시점에 1회 spawn된다.
- 현재 selection은 `EcsInputSnapshot.cursor_screen_pos`와 current quarter-view camera basis를 바탕으로 world raycast를 수행해 `hovered_block`, `hovered_face`, `hit_point`를 채운다.
- 앞/뒤 타겟 전환, 배치 프리뷰 위치 분리, hover `0.3s` 규칙은 아직 future work다.
