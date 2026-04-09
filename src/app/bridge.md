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
- Quarter-view direction rules and follow pose rules are defined once in `ecs::camera` and reused by the bridge so movement, selection, and rendering stay aligned:
  - east projects to screen bottom-right
  - north projects to screen top-right
  - world up projects upward on screen
- bridge는 local player transform만 보고 독자적인 follow camera를 다시 계산하지 않는다

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
- renderer는 여전히 explicit 45-degree downward quarter-view orthographic pose만 받지만, 그 pose의 loose follow / deadzone / bias / recenter 해석은 ECS camera state 쪽에서 끝나 있어야 한다
- 현재 코드는 아직 camera pose를 local player transform에서 직접 재구성하지만, 목표 구조에서는 ECS가 만든 camera snapshot을 bridge가 그대로 번역한다
