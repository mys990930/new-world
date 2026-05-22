# bridge_scene

## Role

- convert app / ECS / world state into render-scene DTOs and upload requests

## Responsibilities

- build `AppRenderFrameData`
- export the ECS-owned quarter-view camera into renderer camera DTOs
- convert selection previews and local player visual state into render cube instances
- convert damaged-block feedback snapshots into subtle shake/tint overlay cube instances
- convert ECS-owned floating block drops into small render cube instances
- convert world CPU meshes into renderer upload payloads
- choose which app-owned UI sprite builders to call for the current top-level app mode

## Inputs

- `GameApp` runtime state
- ECS camera / player / selection / local-environment / damaged-block / floating-drop snapshots
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
- damaged-block rules stay ECS-owned; this layer only maps HP fraction and recent-hit age into visual shake/tint
- floating-drop lifetime and pickup stay ECS-owned; this bridge only exports their current block cube preview
- voxel-player part centers are composed from ECS-facing octants so the visible avatar turns with movement direction without resizing axis-aligned part cubes
- HUD/environment text remains ECS-derived data even though the actual sprite layout stays app-owned

## Invariants

- in-game scene export must keep player visual parts, selection previews, and visible chunk list aligned to the same frame snapshot
- bridge_scene must not emit app-authored ground-shadow quads; dynamic actor cubes rely on renderer-owned shadow-map rendering
- floating block drops use world block face texture layers and material mapping, but they do not mutate or own world block data
- damaged block feedback uses the target block's own texture layers with a translucent damage tint; it is not a second world mesh or source-of-truth edit
- world-select mode must emit UI-only frames without scene cubes or visible chunks
- renderer-facing material mapping must not re-own world semantics beyond the DTO conversion step

## Related Modules

- `bridge.md`
- `bridge_ui.md`
- `frame.md`
- `../ecs/camera.md`
- `../ecs/selection.md`
- `../world/meshing.md`
