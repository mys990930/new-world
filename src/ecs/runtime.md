# runtime

## Role

- Own the ECS runtime.
- Hold the `bevy_ecs::World` and frame/fixed schedules.

## Owned Data

### EcsRuntime
- `World`
- `pre_update`
- `update`
- `post_update`
- `fixed_update`

## Inputs

- bootstrap-time resource/system registration
- app-injected `EcsInputSnapshot`
- app-owned phase calls
- app-owned world/viewport selection and motion helper calls

## Outputs

- updated ECS world/resource state
- discrete command buffer
- `MoveWorldIntent`
- current camera follow state
- `SelectionState`
- local player body/transform snapshot for app bridge

## Process

1. initialize the ECS world and schedules
2. register core resources
3. register systems for `pre`, `update`, `post`, and future `fixed`
4. let app drive the phase boundaries
5. let app call world-aware helpers for:
   - local player collision / gravity motion
   - selection raycast updates

## Current Implementation Notes

- currently registered core resources:
  - `EcsInputSnapshot`
  - `PlayerCommandBuffer`
  - `MoveWorldIntent`
  - `FrameDeltaSeconds`
  - `PlayerMovementConfig`
  - `CameraState`
  - `LocalPlayerEntity`
  - `ChunkStates`
  - `SelectionState`
- current frame schedule:
  - pre: clear command buffer, clear frame camera impulses
  - update: input interpretation -> camera command application -> camera zoom input application -> move intent generation -> local player horizontal velocity sync
  - post: camera follow update
- current world-aware helpers are intentionally outside pure ECS systems because `WorldCore` stays app-owned:
  - `simulate_local_player_motion(&WorldCore)`
  - `update_selection_from_world(&WorldCore, viewport_width, viewport_height)`
  - `place_local_player_on_surface(&WorldCore, anchor_xz)`

## Public Interface
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
EcsRuntime::local_player_body() -> Option<PlayerBody>
EcsRuntime::local_player_physics_state() -> Option<PlayerPhysicsState>
EcsRuntime::simulate_local_player_motion(world: &WorldCore)
EcsRuntime::place_local_player_on_surface(world: &WorldCore, anchor_xz: [f32; 2]) -> bool
EcsRuntime::update_selection_from_world(
    world: &WorldCore,
    viewport_width: u32,
    viewport_height: u32,
)
EcsRuntime::selection_state() -> SelectionState
EcsRuntime::plan_chunk_job_requests(
    world: &WorldCore,
    created_world: Option<&CreatedWorldSource>,
) -> Vec<JobRequest>
EcsRuntime::apply_job_result(result: &JobResult)
EcsRuntime::visible_chunks() -> Vec<ChunkCoord>
```

## Dependencies

- `bevy_ecs`
- `input.rs`
- `command.rs`
- `camera.rs`
- `player.rs`
- `selection.rs`
- `chunk.rs`
- `jobs.rs`

## Invariants

- runtime owns phase order and schedule ownership
- gameplay interpretation remains in leaf ECS systems/helpers
- frame phase and fixed phase remain separate
- world-aware motion and selection helpers only run after app has applied the latest world/job results

## Non-Responsibilities

- raw OS input capture
- world source-of-truth ownership
- jobs execution
- renderer draw

## Related Modules

- `mod.rs`
- `input.rs`
- `command.rs`
- `camera.rs`
- `player.rs`
- `selection.rs`
