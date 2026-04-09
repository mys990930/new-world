# bridge

## Role

- Translate cross-module state at runtime boundaries
- Keep module-owned data models from leaking into neighboring modules

## Responsibilities

- Convert platform raw state into `EcsInputSnapshot`
- Convert ECS gameplay state into render-ready DTOs
- Convert future world/jobs outputs into renderer upload requests

## Non-Responsibilities

- Owning gameplay state transitions
- Mutating world source-of-truth data
- Encoding renderer draw commands directly
- Defining network protocol payloads

## Inputs

- `Platform` state
- `EcsRuntime` state
- `world::CpuMesh` and chunk coord

## Outputs

- `EcsInputSnapshot`
- `AppRenderFrameData`
- `RenderUploadRequest`

## Boundary Rules

- `bridge` stays a mostly stateless translator
- Input meaning belongs to ECS, not to `bridge`
- Renderer receives render-ready DTOs only, never ECS/world internals

## Invariants

- `platform -> ecs` only maps raw transient/held input into frame input resources
- `ecs -> renderer` only maps camera pose, visibility, and draw-ready instances
- Renderer does not query ECS `Transform` or local player entities directly
- Quarter-view direction rules are defined once in `ecs::camera` helper functions and reused by the bridge so movement, selection, and rendering stay aligned:
  - east projects to screen bottom-right
  - north projects to screen top-right
  - world up projects upward on screen

## Related Modules

- `frame.rs`
- `platform`
- `ecs`
- `renderer`

## Notes

- The current vertical slice uses three active bridge paths:
  - `platform -> EcsInputSnapshot`
  - `ecs -> AppRenderFrameData`
  - `world/jobs -> RenderUploadRequest`
- `AppRenderFrameData` currently carries `camera`, `visible_chunks`, and `cube_instances`
- `cube_instances` currently include:
  - the white local player cube
  - a thin dark ground shadow slab
  - a thin yellow face-highlight slab generated from `SelectionState`
- The current prototype uses an explicit 45-degree downward quarter-view basis plus orthographic projection so the renderer can receive a stable render-only camera pose
