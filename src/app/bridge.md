# bridge

## Role

- Translate cross-module state at runtime boundaries.
- Keep module-owned data models from leaking into neighboring modules.

## Responsibilities

- convert platform raw state into `EcsInputSnapshot`
- convert app/ECS gameplay state into render-ready DTOs
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

## Invariants

- `platform -> ecs` only maps raw transient/held input into frame input resources
- `app/ecs -> renderer` only maps camera pose, visibility, draw-ready instances, and app-owned UI rectangles
- `world/jobs -> renderer` copies render-facing mesh payloads without re-owning world semantics
- quarter-view basis rules are still defined in ECS camera code
- the player render body uses ECS-owned `PlayerBody.half_extents`, not a renderer-owned hardcoded size
- quarter-view zoom also stays ECS-owned; the bridge only forwards wheel delta into `EcsInputSnapshot` and later reads the current ECS camera zoom when building `RenderCameraState`
- when moving voxel entities gain richer render DTOs, the bridge should convert ECS-owned continuous facing / pose state into renderer-facing `facing_octant` and `pose_id` fields rather than passing raw gameplay yaw through unchanged

## Related Modules

- `frame.rs`
- `platform`
- `ecs`
- `renderer`

## Notes

- world-side mesh vertices carry `uv`, `texture_layer`, and `material_kind`; the bridge copies or maps all three into renderer upload vertices
- `RenderCubeInstance` also carries a renderer material kind so the player body, ground shadow slab, and hovered-face highlight can be shaded differently
- the ground shadow slab now scales from the ECS player body footprint instead of assuming a unit cube
- weak-perspective quarter-view framing now comes from ECS camera state instead of a renderer-side fixed constant, and the bridge exports the gameplay camera through `RenderProjectionMode::Perspective`
- app-owned HUD and menu layouts currently translate into renderer `RenderUiRect` values, so placeholder screens can be built without renderer knowledge of ECS/world structures
- the preferred future path for moving voxel creatures is: ECS keeps continuous motion/facing, then `bridge` emits a render-facing octant plus pose identifier so renderer stays gameplay-agnostic
