# bridge_scene

## Role

- convert app / ECS / world state into render-scene DTOs and upload requests

## Responsibilities

- build `AppRenderFrameData`
- export the ECS-owned quarter-view camera into renderer camera DTOs
- convert selection previews and local player body state into render cube instances
- convert world CPU meshes into renderer upload payloads
- choose which app-owned UI sprite builders to call for the current top-level app mode

## Inputs

- `GameApp` runtime state
- ECS camera / player / selection snapshots
- `WorldCore`
- current app mode and minimap cache viewport
- `world::CpuMesh`

## Outputs

- `AppRenderFrameData`
- `RenderUploadRequest`

## Boundary Rules

- camera policy still stays ECS-owned; this layer only exports the current render-facing pose
- world mesh semantics stay world-owned until they are copied into renderer DTOs
- selection preview rules stay ECS-owned even though preview cubes are emitted here

## Invariants

- in-game scene export must keep player body, ground shadow, selection previews, and visible chunk list aligned to the same frame snapshot
- world-select mode must emit UI-only frames without scene cubes or visible chunks
- renderer-facing material mapping must not re-own world semantics beyond the DTO conversion step

## Related Modules

- `bridge.md`
- `bridge_ui.md`
- `frame.md`
- `../ecs/camera.md`
- `../ecs/selection.md`
- `../world/meshing.md`
