# ui

## Role

- Define renderer-owned screen-space UI DTOs and atlas-texture resources.

## Responsibilities

- describe app-provided screen-space sprite quads
- own the renderer-local UI vertex layout
- own UI atlas texture loading and GPU bind-group creation

## Owned Data

- `RenderUiTextureSource`
- `RenderUiSprite`
- `RenderUiTextureError`
- internal `UiTextureSet`
- internal `GpuUiTextureResources`
- internal `UiVertex`

## Inputs

- app-owned UI layout data translated by `app::bridge`
- renderer-owned UI atlas texture source

## Outputs

- renderer-consumable screen-space sprite DTOs
- GPU-ready sampled UI texture resources

## Invariants

- UI sprites use pixel screen-space coordinates with origin at the top-left
- UVs are atlas-normalized and renderer-local
- renderer UI DTOs stay gameplay-agnostic and do not expose ECS/world types

## Non-Responsibilities

- layout policy
- hit testing
- world-select ownership
- text shaping beyond atlas-backed sprite glyph lookup performed by `app::bridge`

## Related Modules

- `frame.rs`
- `surface.rs`
- `state.rs`
- `../app/bridge.md`

## Notes

- the current UI path samples a single nearest-filtered pixel atlas
- the current app bridge composes menu panels, tiny bitmap-font text, pre-rendered build quickslot block icons, and tool quickslot icons by emitting `RenderUiSprite` quads
- the fallback texture remains a single white texel so missing atlas files degrade safely
- in-game HUD bars, inventory windows, and world-select menus all use the same atlas-backed sprite path rather than mixing vector/text-system UI approaches
