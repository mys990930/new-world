# camera

## Role

- Own the gameplay-facing quarter-view follow camera state
- Define the shared quarter-view basis and follow pose used by movement, selection, and render bridging

## Owned Data

### `CameraState`

- `quarter_turns`
- `smoothed_target`
- `desired_target`
- `recenter_requested`
- `recentering`
- `initialized`

### `QuarterViewBasis`

- `right`
- `up`
- `forward`

### `QuarterViewCameraPose`

- `target`
- `eye`
- `basis`

## Inputs

- `RotateCamera`
- `RecenterCamera`
- local player `Transform`
- `MoveWorldIntent`
- frame delta time

## Outputs

- the current quarter-view follow target
- the current quarter-view camera pose
- orientation data used by movement intent remapping
- orientation data used by selection ray construction
- a render-ready camera snapshot consumed through `app::bridge`

## State Transition Rules

- The camera is limited to four quarter-view rotations.
- `Q/E` rotation applies before the same frame's movement intent is interpreted.
- The player is followed through a smoothed target rather than a hard snap.
- Deadzone logic is evaluated in quarter-view screen space defined by `right` and `up`.
- Forward movement bias shifts framing toward travel direction without replacing the player-centered anchor.
- Recenter keeps the current rotation and smoothly moves the target back toward the player anchor.
- Selection and render bridging must consume the same smoothed target and basis in a given frame.

## Coordinate Rules

- Window-space intent does not map 1:1 to world axes; ECS owns that interpretation.
- The shared quarter-view basis is the source of truth for movement remapping, selection rays, and render camera pose.
- Orthographic framing is controlled primarily by `QUARTER_VIEW_VERTICAL_WORLD_SIZE`.
- In the current implementation, increasing `QUARTER_VIEW_VERTICAL_WORLD_SIZE` shows more world and makes the camera feel farther away.
- `QUARTER_VIEW_CAMERA_DISTANCE` controls the eye offset along the quarter-view forward axis. Under orthographic projection it affects eye-space relationships such as fog or shadow math more than visible zoom scale.

## Invariants

- ECS owns quarter-view follow policy; renderer only consumes the final pose.
- `quarter_view_basis()` and `quarter_view_camera_pose()` are shared helpers so movement, selection, and rendering stay aligned.
- The bridge must not rebuild a separate gameplay camera interpretation from raw player state.
- Camera tuning should stay in ECS unless the change is purely GPU-side math.

## Non-Responsibilities

- collecting raw OS input
- mutating world source-of-truth data
- computing renderer GPU matrices
- providing a free-fly camera

## Related Modules

- `command.rs`
- `player.rs`
- `selection.rs`
- `chunk.rs`
- `app/bridge.rs`

## Notes

- The current implementation is still a quarter-view follow camera rather than a full strategy-camera system.
- The main zoom/framing handle lives in `src/ecs/camera.rs` as `QUARTER_VIEW_VERTICAL_WORLD_SIZE`.
- The default framing now uses a wider orthographic size so the player sees more surrounding terrain at once.
