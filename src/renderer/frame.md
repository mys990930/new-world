# frame

## Role

- Encode renderer frame passes and perform submit/present

## Responsibilities

- update frame stats
- update GPU camera state
- upload the camera uniform
- upload the environment uniform
- acquire the surface texture
- clear and use the depth buffer
- bind the block texture array
- draw uploaded chunk meshes for the current `visible_chunks`
- expand `cube_instances` into a cube mesh with per-face normals
- draw the terrain pass
- draw the dynamic cube pass
- optionally draw the debug edge overlay pass
- submit and present
- propagate recoverable surface errors

## Non-Responsibilities

- creating render DTOs
- calculating chunk visibility
- interpreting gameplay state

## Inputs

- `RenderFrameInput`

## Outputs

- `RenderStats`
- `RenderError`

## Processing Flow

1. Update frame index and stats.
2. Update `RenderCameraState` into `CameraGpuState`.
3. Resolve the clear color from the active renderer environment unless the app overrides it.
4. Upload camera and environment uniforms.
5. Draw terrain chunk meshes with texture layers plus material kinds.
6. Draw dynamic cube instances with renderer material kinds such as actor, shadow, and highlight.
7. Submit and present.

## Invariants

- renderer only sees render-ready DTOs such as `RenderCubeInstance`
- if there is no live backend or the surface is not configured, nothing is presented
- terrain and dynamic cubes share bind groups but not shader logic

## Related Modules

- `camera.rs`
- `surface.rs`
- `state.rs`

## Notes

- The terrain shader now tries to preserve texture readability before atmosphere/fog grading is applied.
- Dynamic cube instances can be tagged as `Actor`, `Shadow`, or `Highlight` so they no longer all shade as the same generic white cube.
