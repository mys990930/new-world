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
- local player entity id
- `WorldCore` for world-aware motion helpers

## Outputs

- `MoveWorldIntent`
- local player `Velocity`
- local player `Transform`
- local player `PlayerPhysicsState`

## State Transition Rules

- screen-relative directions do not match world east/north directly
- in the default quarter-view:
  - screen top-right = world north
  - screen bottom-right = world east
- movement input is first interpreted in that skewed quarter-view basis
- `CameraState.quarter_turns` is then applied to produce the current world-relative movement intent
- if `RotateCamera` and movement happen in the same frame, movement uses the post-rotation basis
- `MoveWorldIntent` is copied into the local player horizontal velocity channels
- vertical velocity is preserved across frames so gravity and falling can accumulate
- inventory-open state blocks movement intent generation and leaves horizontal velocity at zero
- world-aware motion then resolves:
  - horizontal movement against solid world blocks
  - one-block automatic step-up
  - two-block obstacle rejection
  - gravity and falling
- created-world load can temporarily stage the local player at the requested spawn x/z and a safe loading height before enough chunk data exists for surface placement

## Invariants

- `Transform.translation` is interpreted as body-center position
- `PlayerBody.half_extents` is currently `[1.0, 2.0, 1.0]`, meaning a `2x2x4` block body
- `MoveWorldIntent` is the continuous world-space movement channel
- discrete actions stay in `PlayerCommandBuffer`
- missing world chunks are treated as blocking in the current collision helper so the player does not walk into unloaded space
- pending created-world spawn placement should be retried by app after streamed chunk jobs mutate `WorldCore`, not by ECS polling disk or jobs directly

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

## Notes

- the current minimal implementation spawns one local player during bootstrap
- bootstrap and created-world reload now stage a spawn anchor first; app snaps the local player onto a safe loaded surface once jobs have streamed enough nearby chunk data into `WorldCore`
- the current default horizontal move speed is `8.0` world units per second
- the current locomotion slice is intentionally minimal: no jump, no slope handling beyond one-block step-up, and no network prediction yet
