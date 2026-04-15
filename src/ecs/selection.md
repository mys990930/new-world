# selection

## Role

- Track which world block and face the cursor is currently hovering
- Expose the minimal gameplay snapshot needed for interaction and render highlighting

## Owned Data

### `SelectionState`

- `hovered_block`
- `hovered_face`
- `hit_point`

## Inputs

- `EcsInputSnapshot.cursor_screen_pos`
- `EcsInputSnapshot.focused`
- `EcsInputSnapshot.active`
- the current `CameraState`
- viewport width and height
- `WorldCore::raycast_blocks(...)`

## Outputs

- hovered block
- hovered face
- hit point
- a selection snapshot that `app::bridge` can convert into a render highlight

## State Rules

- Selection updates after app/world state has been refreshed for the current frame.
- The selection ray uses the same smoothed follow target and current zoom size that rendering uses.
- During the current turn-transition implementation, selection still uses the snapped gameplay quarter-view basis while rendering may briefly ease toward that basis.
- The current gameplay slice uses a weak perspective ray that starts at the camera eye and passes through a cursor-selected point on the quarter-view focus plane.
- If focus, activity, viewport, or cursor validity checks fail, selection is cleared.
- If raycast misses, selection is cleared.

## Invariants

- selection owns hover state only; it does not edit the world
- selection does not own world raycast algorithms and uses world queries instead
- renderers do not read `SelectionState` directly; `app::bridge` converts it into render-ready instances
- selection and rendering must stay aligned on follow target and zoom in the same frame, even if render-only turn easing is active

## Non-Responsibilities

- raw input capture
- world mutation
- renderer highlight shading
- hover dwell-time policy

## Related Modules

- `camera.rs`
- `runtime.rs`
- `world`
- `app/bridge.rs`

## Notes

- Zooming the camera now changes the focus-plane selection footprint automatically because selection reads the ECS-owned current zoom value.
