# bridge

## Role

- Translate cross-module state at runtime boundaries.
- Keep module-owned data models from leaking into neighboring modules.

## Responsibilities

- convert platform raw state into `EcsInputSnapshot`
- convert app / ECS gameplay state into render-ready DTOs
- convert app-owned player occlusion targets into renderer-ready block fade DTOs
- convert ECS-owned voxel-player visual state into render-ready dynamic cube DTOs
- convert app-owned menu / HUD layout into renderer-owned pixel-sprite DTOs
- convert cached minimap viewport data into minimap sprite cells
- convert world/jobs outputs into renderer upload requests

## Non-Responsibilities

- owning gameplay state transitions
- mutating world source-of-truth data
- encoding renderer draw commands directly
- defining network protocol payloads

## Inputs

- `Platform` state
- `AppUiState`
- `AppMinimapViewport`
- `EcsRuntime` state
- `world::CpuMesh` and chunk coord

## Outputs

- `EcsInputSnapshot`
- `AppRenderFrameData`
- `RenderUploadRequest`

## Submodules

- `bridge.rs`: shared bridge DTO definitions such as `AppRenderFrameData`
- `bridge_input.rs`: `platform -> ecs` snapshot conversion
- `bridge_scene.rs`: scene-frame and mesh-upload DTO conversion
- `bridge_ui.rs`: minimap, HUD, inventory, and world-select UI sprite conversion

## Boundary Rules

- `bridge` stays mostly stateless
- input meaning belongs to ECS, not to `bridge`
- renderer receives render-ready DTOs only, never ECS/world internals
- renderer UI DTOs stay screen-space sprite data, not app/world/UI ownership data

## Invariants

- `platform -> ecs` only maps raw transient/held input into frame input resources
- `platform -> ecs` also splits wheel input into `Ctrl + wheel = zoom` and plain wheel = quickslot cycling
- `Q/E` raw key presses are normalized here into the ECS camera-rotation axis sign convention
- held Shift is passed through as sprint state; ECS player logic decides how that affects speed
- Space just-pressed state is passed through as jump state; ECS player logic decides whether the local player is grounded and allowed to launch
- `app/ecs -> renderer` only maps camera pose, visibility, draw-ready instances, and app-owned UI sprites
- player occlusion fade targets are presentation-only block coordinates; they must not imply world edits, remeshes, or chunk lifecycle requests
- `world/jobs -> renderer` copies render-facing mesh payloads without re-owning world semantics
- quarter-view basis rules are still defined in ECS camera code
- render camera turn easing is authored in ECS camera state; `bridge` only exports the current render-facing pose
- the current player render body uses ECS-owned `VoxelPlayerVisualState` plus code-authored rig part poses, with app composing them into renderer DTOs
- player ground shadows are not app-authored quads; actor cube instances participate in the renderer shadow-map path
- UI text and panels are built from atlas-backed sprite pieces, not renderer-owned text shaping
- dynamic gameplay preview cubes may choose block face texture layers here, but the preview coordinate/range rules still stay ECS-owned

## Related Modules

- `frame.rs`
- `ui.rs`
- `platform`
- `ecs`
- `renderer`

## Notes

- world-side mesh vertices carry `uv`, `texture_layer`, and `material_kind`; the bridge copies or maps all three into renderer upload vertices
- `RenderCubeInstance` also carries a renderer material kind plus block-face texture layers so actor body parts and gameplay previews can be shaded differently without leaking world ownership into renderer
- the initial voxel-player integration expands the ECS rig into multiple dynamic cube instances in `bridge_scene.rs`, applies simple idle/walk/sprint/airborne offsets, and rotates local part offsets using ECS-facing octants while keeping per-part extents stable; if rotated limbs are required later, bridge should target a renderer DTO extension rather than resizing axis-aligned cubes
- the current world-select UI uses renderer-owned sprite DTOs composed from a pixel atlas, larger atlas-backed bitmap text, editable field rows, a scrollable created-world list, explicit action buttons, and a centered loading popup
- the renderer consumes the same app-owned world-select layout geometry that `ui.rs` uses for mouse hit testing, field focus, list-row selection, and popup blocking, so visible controls and clickable bounds stay aligned
- app-owned HUD and menu layouts no longer emit flat rectangles; they emit sprite quads with atlas UVs and tint only
- in-game inventory HUD now follows the same atlas-backed sprite path: bridge reads ECS inventory snapshots and emits only screen-space sprite DTOs plus render-ready preview cubes
- in-game scene export includes a bounded terrain occlusion block list derived before rendering, allowing the renderer to fade player-covering terrain while keeping chunk meshes unchanged
- the minimap overlay now uses the same pixel-sprite path, but bridge only reads app-owned cached viewport data; minimap chunk-column derivation happens earlier through jobs and cache composition
- bridge is now physically split so raw input mapping, scene DTO export, and UI sprite export can evolve independently without re-growing one monolithic file
