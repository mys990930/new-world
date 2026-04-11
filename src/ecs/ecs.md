## ecs

### Role

- central gameplay state-transition layer
- interpret frame input into gameplay meaning and derive follow-up world/jobs/simulation work
- keep multiplayer-friendly command / intent boundaries

### Responsibilities

- frame/tick phase state transitions
- input interpretation
- player entity/component management
- camera state management
- selection state management
- chunk meta-state management
- jobs result interpretation
- schedule ordering

### Non-Responsibilities

- raw OS event capture
- world source-of-truth block ownership
- direct chunk I/O
- procedural generation algorithms
- meshing algorithms
- main loop ownership

### Owned Data
#### Resources

- `EcsInputSnapshot`
- `PlayerCommandBuffer`
- `MoveWorldIntent`
- `FrameDeltaSeconds`
- `PlayerMovementConfig`
- `CameraState`
- `LocalPlayerEntity`
- `ChunkStates`
- `SelectionState`

#### Entities / Components

- `Player`
- `Transform`
- `Velocity`
- `PlayerBody`
- `PlayerPhysicsState`

#### Discrete Commands

- `PrimaryAction`
- `PlaceBlock`
- `RotateCamera`
- `RecenterCamera`

### Use Cases

- raw input to gameplay meaning
  - `WASD` becomes screen-relative movement state
  - mouse wheel becomes frame-local zoom intent for the quarter-view camera
  - left click requests primary action
  - right click requests block placement
  - `Q/E` request quarter-turn camera rotation
  - `Y` requests camera recenter
- movement intent generation
  - screen-relative input remains frame input state
  - `CameraState.quarter_turns` is applied before generating `MoveWorldIntent`
  - same-frame rotation and movement use the post-rotation basis
- camera framing
  - quarter-view zoom is owned by ECS camera state rather than renderer config
  - the app bridge and selection path both consume the same current zoom size
- player locomotion
  - horizontal velocity derives from `MoveWorldIntent`
  - world-aware motion resolves `2x2x4` body collision, one-block step-up, two-block blocking, and falling
- chunk acquisition planning
  - baked worlds prefer disk load through jobs
  - fallback worlds prefer procedural generation
- selection update
  - app provides cursor position and viewport
  - ECS uses quarter-view camera state to build the selection ray

### Public Interface
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
EcsRuntime::simulate_local_player_motion(world: &WorldCore)
EcsRuntime::place_local_player_on_surface(world: &WorldCore, anchor_xz: [f32; 2]) -> bool
EcsRuntime::update_selection_from_world(
    world: &WorldCore,
    viewport_width: u32,
    viewport_height: u32,
)
EcsRuntime::selection_state() -> SelectionState
```

### Dependencies

- `bevy_ecs`
- `world`
- `simulation`
- `jobs`

### Invariants

1. ECS does not directly read raw OS events
2. world source-of-truth block data stays in `world`
3. discrete actions and continuous movement stay on different channels
4. screen-relative input and world-relative movement intent stay as separate boundaries
5. world-aware helpers may query `WorldCore`, but ECS still does not own world storage

### Submodules
- mod.rs: public facade, re-export
- runtime.rs: `EcsRuntime`, schedule ownership, helper entry points
- input.rs: `EcsInputSnapshot`, frame input resource, discrete command creation
- command.rs: `PlayerCommand`, `MoveWorldIntent`, ECS-side command/request buffers
- player.rs: local player components, `2x2x4` body definition, safe spawn, minimal locomotion
- camera.rs: quarter-view camera state, follow/recenter policy, shared basis helpers
- selection.rs: world-raycast-based hover target state and selection rules
- chunk.rs: chunk interest / acquisition / render-ready meta state
- jobs.rs: jobs result interpretation and deterministic follow-up requests
- fixed.rs: future fixed-tick simulation flow

### Current Implementation Notes

- the current minimal slice now supports both baked-world loading and procedural fallback
- continuous locomotion now runs through a world-aware helper after ECS `update` and before ECS `post_update`
- hover front/back switching, placement preview separation, and network prediction are still future work
