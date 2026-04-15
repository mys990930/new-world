# input

## Role

- Convert the app-provided platform snapshot into an ECS-owned frame input resource
- Keep discrete commands and continuous held state separated before gameplay systems interpret them

## Owned Data

### `EcsInputSnapshot`

- `move_screen_x`
- `move_screen_y`
- `zoom_scroll_delta`
- `primary_down`
- `primary_just_pressed`
- `secondary_down`
- `secondary_just_pressed`
- `rotate_camera`
- `recenter_camera`
- `cursor_screen_pos`
- `cursor_screen_delta`
- `focused`
- `active`

## Inputs

- the frame snapshot produced by `app::bridge`

## Outputs

- discrete commands pushed into `PlayerCommandBuffer`
- continuous movement / camera input state consumed by other ECS systems

## State Rules

- `move_screen_x` and `move_screen_y` remain screen-relative intent state rather than discrete commands
- `primary_just_pressed` becomes `PrimaryAction`
- `secondary_just_pressed` becomes `PlaceBlock`
- `rotate_camera` becomes `RotateCamera`
- the app bridge maps `Q` and `E` into opposite-signed quarter turns so the on-screen turn direction feels natural
- `recenter_camera` becomes `RecenterCamera`
- `zoom_scroll_delta` is left as frame-local continuous camera input and is consumed by `camera.rs`
- when `active == false` or `focused == false`, gameplay commands are not produced

## Frame Boundary Rules

- The app replaces the snapshot once per frame with the latest platform state.
- `*_just_pressed` values are valid only for the current frame.
- hold state remains in `*_down` and movement/scroll fields.

## Invariants

- ECS input does not collect raw OS events directly
- input interpretation does not mutate world state directly
- zoom intent stays inside ECS camera ownership rather than leaking into renderer policy

## Non-Responsibilities

- raw OS event capture
- world-axis remapping
- camera follow / smoothing rules
- selection ray construction

## Related Modules

- `app/bridge.rs`
- `command.rs`
- `player.rs`
- `camera.rs`

## Notes

- The current camera zoom path is `platform wheel delta -> app bridge -> EcsInputSnapshot.zoom_scroll_delta -> camera.rs`.
- Discrete recenter/rotate requests and continuous scroll zoom intentionally stay on different channels.
