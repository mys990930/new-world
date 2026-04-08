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
- Future `JobResult` and world dirty state

## Outputs

- `EcsInputSnapshot`
- `AppRenderFrameData`
- Future `RenderUploadRequest`

## Boundary Rules

- `bridge` stays a mostly stateless translator
- Input meaning belongs to ECS, not to `bridge`
- Renderer receives render-ready DTOs only, never ECS/world internals

## Invariants

- `platform -> ecs` only maps raw transient/held input into frame input resources
- `ecs -> renderer` only maps camera pose, visibility, and draw-ready instances
- Renderer does not query ECS `Transform` or local player entities directly
- Quarter-view camera basis is owned by the bridge and kept consistent with the current prototype mapping:
  - east projects to screen bottom-right
  - north projects to screen top-right
  - world up projects upward on screen

## Related Modules

- `frame.rs`
- `platform`
- `ecs`
- `renderer`

## Notes

- The current vertical slice uses two active bridge paths:
  - `platform -> EcsInputSnapshot`
  - `ecs -> AppRenderFrameData`
- `AppRenderFrameData` currently carries `camera`, `visible_chunks`, and `cube_instances`
- The local player body-center transform is translated into a single `RenderCubeInstance`
- The current prototype uses an explicit 45-degree downward quarter-view basis plus orthographic projection so the renderer can receive a stable render-only camera pose
