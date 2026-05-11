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
- active simulation region calculation for time/season/weather progression
- active chunk-scope calculation for textmode and later entity/ecology simulation observers
- gameplay consumption of world calendar, local weather, and seasonal state
- player-local environment snapshot derivation for HUD / ambience bridges
- gameplay/entity-facing consumption of structured simulation events
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
- `ChunkLifecyclePlan`
- `SelectionState`
- `LocalEnvironmentStatus`
- `SimClock`
- `ActiveSimRegion`
- `SimulationControlState`
- `PendingSimulationResults`

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
  - normal walking currently targets `7` blocks/s
  - holding Shift sprints at `11` blocks/s
  - road speed and difficult-terrain slowdown remain future world/material-aware movement policies
- time / season / weather consumption
  - ECS does not own the authoritative world calendar or season state
  - ECS fixed-phase logic selects the active simulation region around the player
  - ECS fixed-phase logic may also expose a chunk-centered active scope such as the `3x3` chunk window used by `new-world-textmode`
  - ECS can request or consume local weather / seasonal state for gameplay, HUD, audio, and renderer bridge output
  - ECS also keeps one player-centered `LocalEnvironmentStatus` snapshot so app HUD code does not need to resample world state directly
  - local environment refresh reads cached region classification only; uncached atlas cells remain an app/jobs warmup concern
  - nearby changes may appear as immediate gameplay/environment feedback, while far-away seasonal changes may remain deferred until their chunks become interesting
- simulation observer/event consumption
  - `new-world-textmode` may read ECS-selected simulation scope and pending structured results to print diagnostics
  - ECS should treat ecology events such as animal spawn, animal fight, carcass creation, grazing, and plant growth as gameplay-facing data that can later become real entity/component changes
  - ECS must not require text output for those events; textmode formatting remains a binary/app adapter concern
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
pub const HORIZONTAL_RETAIN_CHUNK_RADIUS: i32

EcsRuntime::run_pre_update()
EcsRuntime::run_update()
EcsRuntime::run_post_update()
EcsRuntime::run_fixed_update()
EcsRuntime::sim_clock() -> SimClock
EcsRuntime::active_sim_region() -> ActiveSimRegion
EcsRuntime::active_chunk_observer_scope() -> ActiveChunkObserverScope
EcsRuntime::enqueue_simulation_results(results)
EcsRuntime::drain_pending_simulation_results() -> Vec<SimulationResult>

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
EcsRuntime::stage_local_player_for_chunk_loading(anchor_xz: [f32; 2], max_chunk_y: i32) -> bool
EcsRuntime::update_local_environment_from_world(world: &WorldCore)
EcsRuntime::local_environment_status() -> Option<LocalEnvironmentSnapshot>
EcsRuntime::update_selection_from_world(
    world: &WorldCore,
    viewport_width: u32,
    viewport_height: u32,
)
EcsRuntime::selection_state() -> SelectionState
EcsRuntime::plan_chunk_lifecycle(
    world: &WorldCore,
    created_world: Option<&CreatedWorldSource>,
) -> ChunkLifecyclePlan
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
9. ECS may decide which regions need eager environmental simulation, but world remains the source of truth for calendar and seasonal state
10. app HUD code may read ECS-local environment snapshots, but those snapshots are derived views over world/simulation truth rather than a new owning layer
11. textmode observer output must consume ECS/world/simulation state through structured boundaries rather than adding text-only gameplay state

### Submodules
- mod.rs: public facade, re-export
- runtime.rs: `EcsRuntime`, schedule ownership, helper entry points
- input.rs: `EcsInputSnapshot`, frame input resource, discrete command creation
- command.rs: `PlayerCommand`, `MoveWorldIntent`, ECS-side command/request buffers
- inventory.rs: player inventory/component state, manipulation mode, quickslot selection, and tool definitions
- player.rs: local player components, `2x2x4` body definition, safe spawn, minimal locomotion
- camera.rs: quarter-view camera state, follow/recenter policy, shared basis helpers
- selection.rs: world-raycast-based hover target state, tool preview, and build preview rules
- environment.rs: player-local climate / weather / biome snapshot for HUD-facing bridges
- chunk.rs: chunk interest / acquisition / render-ready meta state
- jobs.rs: jobs result interpretation and deterministic follow-up requests
- fixed.rs: fixed-tick simulation bridge resources and region selection

### Current Implementation Notes

- the current minimal slice now supports both created-world loading and procedural fallback
- continuous locomotion now runs through a world-aware helper after ECS `update` and before ECS `post_update`
- future moving voxel entities should prefer continuous gameplay motion with render-only 8-direction export, because that keeps gameplay math smooth while preserving quarter-view readability
- chunk render-readiness is driven by interest-wide meshing requests, so loaded lower/upper created-world chunks do not stay selectable-but-invisible
- created-world reload may stage the player at the requested spawn x/z before chunks are resident; app clears that pending state after streamed load results allow surface placement
- chunk lifetime now distinguishes `interest` from a broader `retain` envelope so load/unload hysteresis prevents edge thrash when the player hovers around a boundary
- stale chunk load/mesh results must be filtered against the current retain/world state before app reinserts chunks or reuploads meshes
- interaction/build preview now exists, but actual block breaking/placement and inventory drag/drop are still future work
- the first fixed-tick slice is now wired: ECS advances `SimClock`, tracks a player-centered `ActiveSimRegion`, and queues simulation results for app/world follow-up handling
- fixed update now also derives a replaceable player-centered `3x3` `ActiveChunkObserverScope`; app uses it to build world-biome ecology inputs without making simulation own the accumulator or scope policy
- time/season/weather ownership still follows the intended split: world owns truth, simulation owns deterministic advancement rules, and ECS owns active-region selection plus gameplay-side consumption boundaries
- the current frame slice now also refreshes one player-local environment snapshot after world-aware motion and job result application so minimap HUD status can read biome/terrain, date/time, weather, temperature, and humidity without giving `app` new world-query ownership
- player-local environment refresh no longer forces region classification generation on the frame thread; it uses cached samples and waits for app-owned region resolve jobs to populate missing atlas cells
- the planned `new-world-textmode` binary should reuse the same fixed-phase boundaries while printing one-second summaries for a `3x3` chunk window: cell biome, weather, surface state, ecology events, and world update records
