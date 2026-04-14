# ui

## Role

- Define renderer-owned screen-space UI DTOs.

## Responsibilities

- describe app-provided screen-space colored rectangles
- keep renderer-local UI vertex layout separate from world mesh vertices

## Owned Data

- `RenderUiRect`
- internal `UiVertex`

## Inputs

- app-owned UI layout data translated by `app::bridge`

## Outputs

- renderer-consumable UI rectangle DTOs

## Invariants

- UI rectangles use normalized viewport coordinates with origin at the top-left
- renderer UI DTOs stay gameplay-agnostic and do not expose ECS/world types

## Non-Responsibilities

- layout policy
- hit testing
- text shaping

## Related Modules

- `frame.rs`
- `surface.rs`
- `../app/bridge.md`

## Notes

- the current UI path is intentionally minimal and only supports flat colored rectangles
- richer HUD elements can later compose multiple `RenderUiRect` values or add separate DTOs without forcing world dependencies
