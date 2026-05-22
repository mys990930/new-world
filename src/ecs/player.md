# player

## Role

- Define player-facing ECS components and movement state transitions.
- Convert screen-relative movement into world-relative movement intent.
- Own the minimal local-player body/physics slice used by the current prototype.

## Owned Data

### Entity / Component

- `Player`
- `Transform`
- `Velocity`
- `PlayerBody`
- `PlayerPhysicsState`

### Resource / Support State

- `LocalPlayerEntity`
- `FrameDeltaSeconds`
- `PlayerMovementConfig`
- `MoveWorldIntent`

## Inputs

- `EcsInputSnapshot`
- `CameraState`
- `FrameDeltaSeconds`
- `PlayerMovementConfig`
- `PlayerCommandBuffer`
- `PlayerInventory`
- `EcsInputSnapshot.jump_just_pressed`
- local player entity id
- `WorldCore` for world-aware motion helpers

## Outputs

- `MoveWorldIntent`
- local player `Velocity`
- local player `Transform`
- local player `PlayerPhysicsState`
- local player visual snapshot through `player_visual.rs`

## State Transition Rules

- screen-relative directions do not match world east/north directly
- in the default quarter-view:
  - screen top-right = world north
  - screen bottom-right = world east
- movement input is first interpreted in that skewed quarter-view basis
- `CameraState.quarter_turns` is then applied to produce the current world-relative movement intent
- if `RotateCamera` and movement happen in the same frame, movement uses the post-rotation basis
- `MoveWorldIntent` is copied into the local player horizontal velocity channels
- base walking speed is `7` blocks/s, matching the current target walking band of `6.4..7.6` blocks/s
- holding Shift uses sprint speed `11` blocks/s, matching the current target sprint band of `9.6..11.6` blocks/s
- pressing Space while grounded applies a vertical launch velocity calculated from gravity and `jump_height_blocks`
- the default jump target is `2` blocks above the takeoff height, using `sqrt(2 * gravity * jump_height_blocks)`
- road speed and difficult-terrain slowdown are documented future policies and are not applied yet
- vertical velocity is preserved across frames so gravity and falling can accumulate
- inventory-open state blocks movement intent generation and jump launch, and leaves horizontal velocity at zero
- world-aware motion then resolves:
  - horizontal movement against solid world blocks
  - one-block automatic step-up
  - two-block obstacle rejection
  - gravity and falling
- created-world load can temporarily stage the local player at the requested spawn x/z and a safe loading height before enough chunk data exists for surface placement

## Invariants

- `Transform.translation` is interpreted as body-center position
- `PlayerBody.half_extents` is currently `[1.0, 2.0, 1.0]`, meaning a `2x2x4` block body
- `PlayerMovementConfig.jump_height_blocks` is currently `2.0`
- `MoveWorldIntent` is the continuous world-space movement channel
- discrete actions stay in `PlayerCommandBuffer`
- missing world chunks are treated as blocking in the current collision helper so the player does not walk into unloaded space
- pending created-world spawn placement should be retried by app after streamed chunk jobs mutate `WorldCore`, not by ECS polling disk or jobs directly
- `PlayerBody` stays gameplay/collision-owned; visual voxel-player proportions and animation state live in `player_visual.rs`

## Non-Responsibilities

- capturing raw keyboard state
- calculating camera follow behavior
- running selection raycasts
- applying world block edits

## Related Modules

- `input.rs`
- `command.rs`
- `camera.rs`
- `chunk.rs`
- `player_visual.rs`

## Notes

- the current minimal implementation spawns one local player during bootstrap
- bootstrap and created-world reload now stage a spawn anchor first; app snaps the local player onto a safe loaded surface once jobs have streamed enough nearby chunk data into `WorldCore`
- the current walking speed is `7.0` world units per second and hold-Shift sprint speed is `11.0` world units per second
- the current locomotion slice is intentionally minimal: no slope handling beyond one-block step-up and no network prediction yet
- the visual-player rig consumes locomotion/physics state without changing this collision helper or `PlayerBody` dimensions
