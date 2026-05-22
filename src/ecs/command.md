# command

## Role

- define the ECS-side boundary between discrete gameplay commands and continuous movement intent
- provide a staging layer between frame input interpretation and stateful gameplay systems

## Owned Data

### `PlayerCommand`
- `PrimaryAction`
- `PlaceBlock`
- `RotateCamera { quarter_turns }`
- `RecenterCamera`
- `ToggleManipulationMode`
- `ToggleInventory`
- `CycleQuickslot { delta }`
- `SelectQuickslot { slot_index }`

### `MoveWorldIntent`
- `east`
- `north`

### `PlayerCommandBuffer`
- the current frame's ordered list of discrete gameplay commands

## Inputs

- discrete commands created by `input.rs`
- command consumers in `camera.rs`, `inventory.rs`, and future gameplay systems

## Outputs

- deterministic discrete commands for camera/inventory/gameplay state transitions
- continuous world-relative movement intent for player locomotion
- a multiplayer-friendly gameplay command boundary that can later feed networking

## Creation Rules

- `MoveScreen` is no longer a command
- screen-relative held movement remains in `EcsInputSnapshot`
- only discrete actions enter `PlayerCommandBuffer`
- `MoveWorldIntent` is derived later by player systems after camera rotation has been applied

## Consumption Rules

- the command buffer is cleared at the frame boundary
- same-frame `RotateCamera` must be applied before `MoveWorldIntent` is generated
- `RecenterCamera` requests camera follow recentering without changing logical quarter-turn state
- inventory and quickslot commands mutate player-owned inventory state without mutating world blocks directly
- `PrimaryAction` is a tool-use request; ECS/tool interaction computes transient damage and app coordinates any resulting world edit
- `PlaceBlock` remains the build-mode placement request consumed by app/world coordination
- `MoveWorldIntent` is continuous state and is not drained like the discrete command buffer

## Order Rules

- command consumption order inside `update` must stay deterministic
- the intended current order is:
  - input interpretation
  - inventory / mode / quickslot command application
  - camera command application
  - camera zoom input application
  - move intent generation
  - local player horizontal velocity sync

## Invariants

- discrete actions and continuous movement intent do not share the same buffer
- `MoveWorldIntent` is world-relative, not window-relative
- multiplayer-facing gameplay boundaries should prefer world-relative intent rather than renderer/view-relative state

## Non-Responsibilities

- raw input capture
- world edit application
- renderer upload

## Related Modules

- `input.rs`
- `inventory.rs`
- `camera.rs`
- `player.rs`

## Notes

- block placement and prototype block breaking now consume `PlaceBlock` / `PrimaryAction` through app-owned world-edit coordination; richer use-item gameplay remains future work
