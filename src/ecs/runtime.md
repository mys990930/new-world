# runtime

## 역할

- ECS 런타임의 소유자
- `bevy_ecs::World`와 각 frame/fixed schedule을 보관하고 실행한다

## 소유 데이터

### EcsRuntime
- `bevy_ecs::World`
- pre_update schedule
- update schedule
- post_update schedule
- fixed_update schedule
- 기본 resource 등록 규칙

## 입력

- bootstrap 시점의 초기 resource / component / system 등록
- app이 주입한 frame 입력 resource
- app이 호출하는 `run_pre_update`, `run_update`, `run_post_update`, `run_fixed_update`

## 출력

- 갱신된 ECS world/resource 상태
- command buffer / simulation request / jobs request 같은 후속 단계 입력

## 처리 흐름

1. runtime이 `World`와 schedule들을 초기화한다
2. 최소 필수 resource를 등록한다
3. 각 phase에 맞는 시스템을 schedule에 등록한다
4. app이 frame/fixed 단계에 맞춰 schedule 실행을 요청한다
5. 실행 결과를 상위 계층이 읽거나 drain한다

## 현재 구현 메모

- 현재 구현은 `EcsInputSnapshot`, `PlayerCommandBuffer`, `LocalPlayerEntity`를 등록하고, command buffer clear + input 해석 시스템만 연결한다
- bootstrap 시점에 호출 가능한 기본 player spawn helper를 제공한다

## 외부 인터페이스

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
```

## 의존성

- bevy_ecs
- input.rs
- command.rs
- 이후 player.rs / camera.rs / selection.rs / chunk.rs / jobs.rs / fixed.rs

## 불변식

- `bevy_ecs` 구체 타입은 runtime 내부에 캡슐화된다
- phase 순서와 schedule ownership은 runtime이 가진다
- runtime은 실행 순서를 소유하지만 gameplay 의미 자체를 만들지는 않는다
- frame phase와 fixed phase는 분리되어 유지된다

## 비책임

- raw OS 입력 해석
- world 원본 데이터 소유
- jobs 실행
- renderer draw 호출
- fixed timestep accumulator 관리

## 관련 모듈

- mod.rs가 외부에 facade를 제공
- input.rs / command.rs / player.rs / camera.rs / selection.rs / chunk.rs / jobs.rs / fixed.rs 시스템을 조립한다
- app이 runtime을 호출해 frame/fixed 순서를 오케스트레이션한다

## 메모

- 시스템 등록이 많아지면 phase별 하위 registration 함수로 다시 분리할 수 있다
- startup phase가 필요해지면 runtime 내부 또는 app bootstrap 단계와의 경계를 다시 잡아야 한다
