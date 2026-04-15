## ecs

### Role

- central gameplay state-transition layer
- interpret frame input into gameplay meaning and derive follow-up world/jobs/simulation work
- keep multiplayer-friendly command / intent boundaries

### Responsibilities

- frame/tick phase state transitions
- input interpretation
- inventory / quickslot / manipulation-mode state management
- player entity/component management
- moving-entity facing / pose state management
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
- `ToolCatalog`
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
- `PlayerInventory`

#### Discrete Commands

- `PrimaryAction`
- `PlaceBlock`
- `RotateCamera`
- `RecenterCamera`
- `ToggleManipulationMode`
- `ToggleInventory`
- `CycleQuickslot`
- `SelectQuickslot`

### Use Cases

- raw input to gameplay meaning
  - `WASD` becomes screen-relative movement state
  - `Ctrl + wheel` becomes frame-local zoom intent for the quarter-view camera
  - plain wheel becomes quickslot cycling for the active manipulation mode
  - left click requests primary action
  - right click requests block placement
  - `Q/E` request quarter-turn camera rotation
  - `Y` requests camera recenter
  - `Tab` requests manipulation-mode toggle
  - `I` requests inventory toggle
  - `1..0` select the active mode quickslot directly
- inventory / mode state
  - the local player owns one inventory component with general slots plus separate tool/block quickslots
  - tool quickslots and block quickslots each hold 10 slots
  - the two quickslot bars keep separate selected indices
  - `interaction` mode uses the tool quickslots
  - `build` mode uses the block quickslots
  - the current shared player reach for interaction/build preview is `6` blocks
  - opening inventory blocks movement, world interaction, and raycast preview updates until it is closed
- movement intent generation
  - screen-relative input remains frame input state
  - `CameraState.quarter_turns` is applied before generating `MoveWorldIntent`
  - same-frame rotation and movement use the post-rotation basis
  - render-only camera turn easing must not delay gameplay basis changes
- camera framing
  - quarter-view zoom is owned by ECS camera state rather than renderer config
  - the app bridge and selection path both consume the same current zoom size
- player locomotion
  - horizontal velocity derives from `MoveWorldIntent`
  - world-aware motion resolves `2x2x4` body collision, one-block step-up, two-block blocking, and falling
- moving entity render-facing state
  - gameplay-facing movement / yaw may stay continuous in ECS
  - render-facing direction for voxel creatures should quantize to 8 octants only when exporting render DTOs
  - octant switching policy, hysteresis, and pose selection stay ECS-owned rather than renderer-owned
- chunk acquisition planning
  - created worlds prefer disk load through jobs
  - fallback worlds prefer procedural generation
  - vertically loaded created-world chunks that are inside the current interest set must still request meshing even when they are below or above the player's current chunk layer
  - when a chunk becomes available, ECS may invalidate adjacent chunk meshes so seam-dependent terrain shading rebuilds with the new neighbor snapshot
- selection update
  - app provides cursor position and viewport
  - ECS uses quarter-view camera state to build the selection ray
  - interaction mode turns a valid raycast hit into a tool-shaped weak red preview volume, or a default single-block preview if no tool is selected
  - build mode turns a valid raycast hit into a translucent placement preview on the adjacent face if it is inside reach and empty, even when the active block quickslot is empty

### Public Interface
```rust
EcsRuntime::new() -> EcsRuntime
EcsRuntime::insert_resource<T>(&mut self, value: T)
EcsRuntime::world(&self) -> &World
EcsRuntime::world_mut(&mut self) -> &mut World

pub const HORIZONTAL_INTEREST_CHUNK_RADIUS: i32

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
EcsRuntime::local_player_inventory() -> Option<PlayerInventory>
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
6. render-facing octant / pose state for moving voxel entities is derived gameplay output, not renderer-authored state
7. the local player inventory stays player-owned ECS state rather than app-owned HUD state
8. inventory-open UI blocking affects gameplay interpretation inside ECS rather than changing renderer ownership

### Submodules
- mod.rs: public facade, re-export
- runtime.rs: `EcsRuntime`, schedule ownership, helper entry points
- input.rs: `EcsInputSnapshot`, frame input resource, discrete command creation
- command.rs: `PlayerCommand`, `MoveWorldIntent`, ECS-side command/request buffers
- inventory.rs: player inventory/component state, manipulation mode, quickslot selection, and tool definitions
- player.rs: local player components, `2x2x4` body definition, safe spawn, minimal locomotion
- camera.rs: quarter-view camera state, follow/recenter policy, shared basis helpers
- selection.rs: world-raycast-based hover target state, tool preview, and build preview rules
- chunk.rs: chunk interest / acquisition / render-ready meta state
- jobs.rs: jobs result interpretation and deterministic follow-up requests
- fixed.rs: future fixed-tick simulation flow

### Current Implementation Notes

- the current minimal slice now supports both created-world loading and procedural fallback
- continuous locomotion now runs through a world-aware helper after ECS `update` and before ECS `post_update`
- future moving voxel entities should prefer continuous gameplay motion with render-only 8-direction export, because that keeps gameplay math smooth while preserving quarter-view readability
- chunk render-readiness is driven by interest-wide meshing requests, so loaded lower/upper created-world chunks do not stay selectable-but-invisible
- interaction/build preview now exists, but actual block breaking/placement and inventory drag/drop are still future work
