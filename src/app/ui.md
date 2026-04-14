# ui

## Role

- Own app-level screen mode and lightweight UI interaction state.

## Responsibilities

- store the current top-level `AppMode`
- store app-owned overlay toggles
- handle screen-level shortcuts that should not belong to ECS

## Owned Data

### AppMode
- `InGame`
- `WorldSelect`

### AppUiState
- current `AppMode`
- minimap overlay visibility flag
- selected world-slot index for the placeholder world-select screen

## Inputs

- platform raw input snapshot as read by `app`

## Outputs

- app-mode transitions
- app-owned overlay toggles

## State Transition Rules

- `F1` toggles between `InGame` and `WorldSelect`
- `Tab` toggles the minimap overlay placeholder
- while `WorldSelect` is open, `Left/Right` moves the selected slot and `Enter/Escape` returns to `InGame`

## Invariants

- screen mode is app-owned rather than ECS-owned
- placeholder world-select interaction does not mutate `world`, `jobs`, or `ecs`

## Non-Responsibilities

- gameplay input interpretation
- renderer draw encoding
- baked-world loading

## Related Modules

- `state.rs`
- `frame.rs`
- `bridge.rs`

## Notes

- the current world-select screen is intentionally a renderer-agnostic placeholder layout made from app-owned UI DTOs
- the minimap overlay is currently a frame-only placeholder and does not yet sample live world data
