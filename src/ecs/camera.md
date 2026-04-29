# camera

## Role

- Own the gameplay-facing quarter-view follow camera state
- Define the shared quarter-view basis and follow pose used by movement, selection, and render bridging

## Owned Data

### `CameraState`

- `quarter_turns`
- `smoothed_target`
- `desired_target`
- `render_yaw_radians`
- `desired_render_yaw_radians`
- `vertical_world_size`
- `desired_vertical_world_size`
- `recenter_requested`
- `recentering`
- `render_rotation_initialized`
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
- Gameplay-facing quarter-turn state still snaps immediately so movement and other ECS interpretation use the new basis in the same frame.
- Render-facing rotation may ease toward the new quarter-turn target over a short visual transition of roughly 200 ms.
- The player is followed through a smoothed target rather than a hard snap.
- Horizontal framing stays loose and deadzone-driven, but the target height should still follow the player's body-center `y`.
- Mouse-wheel zoom updates the desired quarter-view focus-plane world size inside ECS camera state.
- Deadzone logic is evaluated in quarter-view screen space defined by `right` and `up`.
- Forward movement bias shifts framing toward travel direction without replacing the player-centered anchor.
- Recenter keeps the current rotation and smoothly moves the target back toward the player anchor.
- Selection and render bridging must consume the same smoothed target and zoom size in a given frame.
- During a short turn transition, selection continues to use the snapped gameplay basis while rendering may use a briefly interpolated visual basis.

## Coordinate Rules

- Window-space intent does not map 1:1 to world axes; ECS owns that interpretation.
- The shared quarter-view basis is the source of truth for movement remapping, selection rays, and render camera pose.
- Gameplay render/selection framing is controlled primarily by `QUARTER_VIEW_VERTICAL_WORLD_SIZE`.
- The runtime camera starts from `QUARTER_VIEW_VERTICAL_WORLD_SIZE` and then lerps `vertical_world_size` toward `desired_vertical_world_size`.
- In the current implementation, increasing `QUARTER_VIEW_VERTICAL_WORLD_SIZE` shows more world and makes the camera feel farther away.
- The gameplay slice now uses a weak perspective projection, so `vertical_world_size` describes the world span around the focal plane rather than a pure orthographic slab.
- `QUARTER_VIEW_PERSPECTIVE_VERTICAL_FOV_RADIANS` and the derived perspective eye distance must stay aligned with the renderer config wired up by app bootstrap.
- `QUARTER_VIEW_CAMERA_DISTANCE` remains a legacy orthographic helper for preview/debug paths that still use orthographic quarter-view tooling.

## Invariants

- ECS owns quarter-view follow policy; renderer only consumes the final pose.
- `quarter_view_basis()` and `quarter_view_camera_pose()` remain the shared gameplay helpers for movement and selection.
- `quarter_view_render_camera_pose()` derives the render-facing pose from the same follow target while allowing a short visual turn transition.
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
- The main gameplay render + selection path now uses a weak perspective quarter-view camera rather than a fully orthographic one.
- Runtime zoom is clamped between a minimum and maximum vertical world size and is driven by mouse-wheel input through ECS.
- The current zoom-out ceiling is `96.0` vertical world units, twice the previous farthest gameplay zoom.
- Follow/recenter interpolation is intentionally very gentle so quarter-view transitions feel less abrupt.
- Vertical camera motion should continue to react to player height changes from steps, slopes, jumps, or falls even while horizontal follow remains deadzone-based.
