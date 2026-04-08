# player

## Role

- Define player-facing ECS components and movement state transitions
- Convert screen-relative movement into world-relative movement intent

## Owned Data

### Entity / Component

- `Player`
- `Transform`
- `Velocity`

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
- Local player entity id

## Outputs

- `MoveWorldIntent`
- Local player `Velocity`
- Local player `Transform`
- Future action state and chunk-interest inputs

## State Transition Rules

- Screen-relative directions do not match world east/north directly
- In the default quarter-view:
  - screen top-right = world north
  - screen bottom-right = world east
- Movement input is first interpreted in that skewed quarter-view basis
- `CameraState.quarter_turns` is then applied to produce the current world-relative movement intent
- If `RotateCamera` and movement happen in the same frame, movement uses the post-rotation basis
- `MoveWorldIntent` is copied into the local player `Velocity`
- The same frame then integrates `Velocity * PlayerMovementConfig.units_per_second * FrameDeltaSeconds` into `Transform.translation`

## Invariants

- `Transform.translation` is interpreted as body-center position
- `MoveWorldIntent` is the continuous world-space movement channel
- Discrete actions stay in `PlayerCommandBuffer`
- The bootstrap local player spawns at body-center `[3.0, 1.5, 3.0]` so a unit debug cube stands on the center of the generated `(1..=5, 1..=5)` chunk patch

## Non-Responsibilities

- Capturing raw keyboard state
- Calculating camera follow behavior
- Running selection raycasts
- Applying world block edits

## Related Modules

- `input.rs`
- `command.rs`
- `camera.rs`
- `chunk.rs`

## Notes

- The current minimal implementation spawns one local player during bootstrap
- The current slice updates both velocity and transform, without collision or gravity yet
