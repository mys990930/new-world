# bridge

## Role

- Translate cross-module state at runtime boundaries.
- Keep module-owned data models from leaking into neighboring modules.

## Responsibilities

- convert platform raw state into `EcsInputSnapshot`
- convert app / ECS gameplay state into render-ready DTOs
- convert app-owned menu / HUD layout into renderer-owned pixel-sprite DTOs
- convert world/jobs outputs into renderer upload requests

## Non-Responsibilities

- owning gameplay state transitions
- mutating world source-of-truth data
- encoding renderer draw commands directly
- defining network protocol payloads

## Inputs

- `Platform` state
- `AppUiState`
- `EcsRuntime` state
- `world::CpuMesh` and chunk coord

## Outputs

- `EcsInputSnapshot`
- `AppRenderFrameData`
- `RenderUploadRequest`

## Boundary Rules

- `bridge` stays mostly stateless
- input meaning belongs to ECS, not to `bridge`
- renderer receives render-ready DTOs only, never ECS/world internals
- renderer UI DTOs stay screen-space sprite data, not app/world/UI ownership data

## Invariants

- `platform -> ecs` only maps raw transient/held input into frame input resources
- `Q/E` raw key presses are normalized here into the ECS camera-rotation axis sign convention
- `app/ecs -> renderer` only maps camera pose, visibility, draw-ready instances, and app-owned UI sprites
- `world/jobs -> renderer` copies render-facing mesh payloads without re-owning world semantics
- quarter-view basis rules are still defined in ECS camera code
- render camera turn easing is authored in ECS camera state; `bridge` only exports the current render-facing pose
- the player render body uses ECS-owned `PlayerBody.half_extents`, not a renderer-owned hardcoded size
- UI text and panels are built from atlas-backed sprite pieces, not renderer-owned text shaping

## Related Modules

- `frame.rs`
- `ui.rs`
- `platform`
- `ecs`
- `renderer`

## Notes

- world-side mesh vertices carry `uv`, `texture_layer`, and `material_kind`; the bridge copies or maps all three into renderer upload vertices
- `RenderCubeInstance` also carries a renderer material kind so the player body, ground shadow slab, and hovered-face highlight can be shaded differently
- the current world-select UI uses renderer-owned sprite DTOs composed from a pixel atlas, larger atlas-backed bitmap text, spinner rows, and explicit action buttons
- the renderer consumes the same app-owned world-select layout geometry that `ui.rs` uses for mouse hit testing, so visible controls and clickable bounds stay aligned
- app-owned HUD and menu layouts no longer emit flat rectangles; they emit sprite quads with atlas UVs and tint only
