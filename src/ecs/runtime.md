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

## 출력

- 갱신된 ECS world/resource 상태
- discrete command buffer
- `MoveWorldIntent`
- app bridge가 읽을 수 있는 최소 gameplay snapshot

## 처리 흐름

1. runtime이 `World`와 schedule을 초기화한다
2. 최소 필수 resource를 등록한다
3. pre/update/post/fixed 각 phase에 시스템을 등록한다
4. app가 각 phase를 호출한다

## 현재 구현 메모

- 현재 등록되는 기본 resource는:
  - `EcsInputSnapshot`
  - `PlayerCommandBuffer`
  - `MoveWorldIntent`
  - `CameraState`
  - `LocalPlayerEntity`
- 현재 frame update 순서는:
  - pre: command buffer clear, camera one-shot impulse clear
  - update: input interpretation -> camera command 적용 -> move world intent 생성 -> local player velocity 반영
- app bridge는 runtime helper를 통해 현재 `CameraState`와 local player `Transform`을 읽어 renderer DTO를 만든다

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
EcsRuntime::camera_state() -> CameraState
EcsRuntime::local_player_transform() -> Option<Transform>
```

## 의존성

- `bevy_ecs`
- `input.rs`
- `command.rs`
- `camera.rs`
- `player.rs`

## 불변식

- runtime은 phase 순서와 schedule ownership을 가진다
- gameplay 해석 자체는 하위 시스템이 담당한다
- frame phase와 fixed phase는 분리 유지한다

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

## 메모

- 현재 `fixed_update`는 문서상 슬롯만 있고 gameplay 내용은 아직 비어 있다
