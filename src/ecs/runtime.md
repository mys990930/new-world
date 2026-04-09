# runtime

## 역할

- ECS 런타임의 소유자
- `bevy_ecs::World`와 frame/fixed schedule을 보관하고 실행한다

## 소유 데이터

### EcsRuntime
- `World`
- `pre_update`
- `update`
- `post_update`
- `fixed_update`

## 입력

- bootstrap 시점의 초기 resource / system 등록
- app가 주입하는 `EcsInputSnapshot`
- app가 호출하는 phase 실행 함수
- app가 넘기는 world/viewport 기반 selection update 요청

## 출력

- 갱신된 ECS world/resource 상태
- discrete command buffer
- `MoveWorldIntent`
- current camera state / follow snapshot
- `SelectionState`
- app bridge가 읽을 수 있는 최소 gameplay snapshot

## 처리 흐름

1. runtime이 `World`와 schedule을 초기화한다
2. 최소 필수 resource를 등록한다
3. pre/update/post/fixed 각 phase에 시스템을 등록한다
4. app가 각 phase를 호출한다
5. app가 world/job 결과 반영 이후 selection helper를 호출하면 runtime이 world raycast 기반 `SelectionState`를 갱신한다

## 현재 구현 메모

- 현재 등록되는 기본 resource는:
  - `EcsInputSnapshot`
  - `PlayerCommandBuffer`
  - `MoveWorldIntent`
  - `FrameDeltaSeconds`
  - `PlayerMovementConfig`
  - `CameraState`
  - `LocalPlayerEntity`
  - `ChunkStates`
  - `SelectionState`
- 현재 frame update 순서는:
  - pre: command buffer clear, frame 단위 camera request 정리
  - target update: input interpretation -> camera command 적용 -> move world intent 생성 -> local player velocity 반영 -> local player transform 적분 -> camera follow state 갱신
- current selection update는 bevy system이 아니라 runtime helper로 분리되어 있다.
  - 이유: world source-of-truth는 ECS 내부가 아니라 app가 소유하므로, `WorldCore` 참조를 직접 받는 지점이 필요하다.
- app bridge는 runtime helper를 통해 현재 `CameraState`, `SelectionState`, visible chunk 목록을 읽어 renderer DTO를 만든다.
- selection은 render와 같은 camera follow pose를 공유해야 하므로, camera 갱신이 끝난 뒤의 snapshot을 읽는다.

## 공개 인터페이스

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
EcsRuntime::set_frame_delta_seconds(dt_seconds: f32)
EcsRuntime::camera_state() -> CameraState
EcsRuntime::local_player_transform() -> Option<Transform>
EcsRuntime::update_selection_from_world(
    world: &WorldCore,
    viewport_width: u32,
    viewport_height: u32,
)
EcsRuntime::selection_state() -> SelectionState
EcsRuntime::plan_chunk_job_requests(world: &WorldCore) -> Vec<JobRequest>
EcsRuntime::apply_job_result(result: &JobResult)
EcsRuntime::visible_chunks() -> Vec<ChunkCoord>
```

## 의존성

- `bevy_ecs`
- `input.rs`
- `command.rs`
- `camera.rs`
- `player.rs`
- `selection.rs`
- `chunk.rs`
- `jobs.rs`

## 불변식

- runtime은 phase 순서와 schedule ownership을 가진다
- gameplay 해석 자체는 하위 시스템이 담당한다
- frame phase와 fixed phase는 분리 유지한다
- world source-of-truth를 직접 읽는 selection update는 app 오케스트레이션 이후에만 실행된다
- runtime이 노출하는 camera snapshot은 selection과 render bridge가 같은 프레임 해석을 공유하도록 유지되어야 한다

## 비책임

- raw OS input 해석
- world 원본 데이터 소유
- jobs 실행
- renderer draw

## 관련 모듈

- `mod.rs`
- `input.rs`
- `command.rs`
- `camera.rs`
- `player.rs`
- `selection.rs`

## 메모

- 현재 `fixed_update`는 문서상 슬롯만 있고 gameplay 내용은 아직 비어 있다.
- 현재 chunk/job 관련 흐름은 player가 서 있는 청크 하나를 deterministic interest 대상으로 삼는 최소 vertical slice다.
- 현재 코드는 아직 dedicated camera follow update 단계가 없고, selection/render camera를 raw player transform 쪽에서 재구성한다.
- 다음 구현은 camera follow 상태를 ECS runtime 쪽에서 갱신하고, bridge/selection은 그 결과를 읽도록 정렬한다.
