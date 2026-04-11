# frame

## Role

- Encode renderer frame passes and perform submit/present

## Responsibilities

- update frame stats
- update GPU camera state
- upload the camera uniform
- upload the environment uniform
- upload the sun-shadow uniform
- acquire the surface texture
- render the shadow depth pass
- clear and use the depth buffer
- draw the visible sun overlay
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
4. Build a renderer-owned sun-shadow uniform from the current camera, sun direction, and visible geometry bounds.
5. Upload camera, environment, and sun-shadow uniforms.
6. Render the shadow map when the active quality preset enables it.
7. Begin the main color pass, draw the visible sun overlay, then draw terrain and dynamic cubes.
8. Optionally draw the debug edge overlay pass.
9. Submit and present.

## Invariants

- renderer only sees render-ready DTOs such as `RenderCubeInstance`
- if there is no live backend or the surface is not configured, nothing is presented
- terrain and dynamic cubes share bind groups but not shader logic
- visible-sun and shadow-map calculations are renderer-local and derive from current render state only
- orthographic fog should be based on focal-area distance rather than raw eye distance, because the quarter-view eye sits far away only to define the view basis

## Related Modules

- `camera.rs`
- `surface.rs`
- `state.rs`

## Notes

- The current shadow solution is a single directional hard-sun map fit to visible terrain/cube bounds.
- The visible sun is a full-screen overlay pass positioned from the current sun direction projected into the active camera.
- Terrain and dynamic shaders both sample the same shadow map, but react differently based on material kind.
- Terrain and dynamic fog now key off the camera focus position, which avoids washing the whole scene just because the orthographic eye offset is large.
