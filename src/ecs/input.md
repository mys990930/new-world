# input

## Role

- Convert the app-provided platform snapshot into an ECS-owned frame input resource
- Keep discrete commands and continuous held state separated before gameplay systems interpret them

## Owned Data

### `EcsInputSnapshot`

- `move_screen_x`
- `move_screen_y`
- `sprint_down`
- `zoom_scroll_delta`
- `quickslot_scroll_steps`
- `primary_down`
- `primary_just_pressed`
- `secondary_down`
- `secondary_just_pressed`
- `rotate_camera`
- `recenter_camera`
- `toggle_manipulation_mode`
- `toggle_inventory`
- `select_quickslot`
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
- `sprint_down` is held state, not a toggle, and is consumed by `player.rs` when choosing the current horizontal speed
- `primary_down` becomes repeated `PrimaryAction` commands while a tool is selected and the tool cooldown is ready
- `secondary_just_pressed` becomes `PlaceBlock`
- `rotate_camera` becomes `RotateCamera`
- the app bridge maps `Q` and `E` into opposite-signed quarter turns so the on-screen turn direction feels natural
- `recenter_camera` becomes `RecenterCamera`
- `toggle_manipulation_mode` becomes `ToggleManipulationMode`
- `toggle_inventory` becomes `ToggleInventory`
- `quickslot_scroll_steps` becomes `CycleQuickslot`
- `select_quickslot` becomes `SelectQuickslot`
- `zoom_scroll_delta` is left as frame-local continuous camera input and is consumed by `camera.rs`
- the app bridge maps plain wheel into quickslot cycling and `Ctrl + wheel` into camera zoom
- when `active == false` or `focused == false`, gameplay commands are not produced, but the internal tool cooldown still advances
- inventory-open UI still allows inventory/mode/quickslot commands to be produced, but movement/world interaction should be suppressed by downstream ECS systems

## Frame Boundary Rules

- The app replaces the snapshot once per frame with the latest platform state.
- `*_just_pressed` values are valid only for the current frame.
- hold state remains in `*_down` and movement/scroll fields.
- held primary input repeats through `ToolUseCooldown` rather than producing a command every frame.

## Invariants

- ECS input does not collect raw OS events directly
- input interpretation does not mutate world state directly
- input interpretation owns only the tool-use cadence; block damage and world edits are handled by `tool_interaction.rs` and app/world coordination
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
- `tool_interaction.rs`

## Notes

- The current zoom path is `platform ctrl+wheel -> app bridge -> EcsInputSnapshot.zoom_scroll_delta -> camera.rs`.
- The current quickslot path is `platform wheel / digit keys -> app bridge -> PlayerCommandBuffer -> inventory.rs`.
- Discrete inventory/mode/slot requests and continuous scroll zoom intentionally stay on different channels.
- The prototype tool cooldown is currently a global `0.5` seconds and is intended to become data-driven later.
