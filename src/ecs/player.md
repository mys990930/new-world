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
- `MoveWorldIntent`

## Inputs

- `EcsInputSnapshot`
- `CameraState`
- `PlayerCommandBuffer`
- Local player entity id

## Outputs

- `MoveWorldIntent`
- Local player `Velocity`
- Future `Transform`, action state, and chunk-interest inputs

## State Transition Rules

- Screen-relative directions do not match world east/north directly
- In the default quarter-view:
  - screen top-right = world north
  - screen bottom-right = world east
- Movement input is first interpreted in that skewed quarter-view basis
- `CameraState.quarter_turns` is then applied to produce the current world-relative movement intent
- If `RotateCamera` and movement happen in the same frame, movement uses the post-rotation basis
- In the current minimal slice, `MoveWorldIntent` is copied directly into the local player `Velocity`

## Invariants

- `Transform.translation` is interpreted as body-center position
- `MoveWorldIntent` is the continuous world-space movement channel
- Discrete actions stay in `PlayerCommandBuffer`
- The bootstrap local player spawns at body-center `[0.0, 0.5, 0.0]` so a unit debug cube stands on ground level `y = 0.0`

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
- The current slice updates velocity only; transform integration is still a later step
