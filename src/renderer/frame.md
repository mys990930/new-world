# frame

## Role

- Encode renderer frame passes and perform submit/present

## Responsibilities

- Update frame stats
- Update GPU camera state
- Upload the camera uniform
- Acquire the surface texture
- Clear and use the depth buffer
- Expand `cube_instances` into a white cube mesh with per-face normals
- Draw the filled cube pass
- Optionally draw the debug edge overlay pass
- Submit and present
- Propagate recoverable surface errors

## Non-Responsibilities

- Creating render DTOs
- Calculating chunk visibility
- Interpreting gameplay state

## Inputs

- `RenderFrameInput`

## Outputs

- `RenderStats`
- `RenderError`

## Processing Flow

1. Update frame index and stats.
2. Update `RenderCameraState` into `CameraGpuState`.
3. If there is no live backend, return stats only.
4. Acquire the surface texture.
5. Upload the camera uniform.
6. Bind the fixed directional-light uniform.
7. Expand `cube_instances` into a white cube mesh with face normals.
8. Run the main pass to clear and draw the filled cube.
9. If `debug_overlay` is enabled, compute visible edges and draw the overlay pass.
10. Submit and present.

## Invariants

- Renderer only sees render-ready DTOs such as `RenderCubeInstance`.
- If there is no live backend or the surface is not configured, nothing is presented.
- Acquire failures are returned as recoverable `RenderSurfaceError` values.

## Related Modules

- `camera.rs`
- `surface.rs`
- `state.rs`

## Notes

- The current implementation prioritizes the minimum dynamic draw path needed to visualize the player cube before chunk drawing exists.
- The cube uses a white base color, per-face normals, and a fixed directional light to make volume readable without gameplay-owned lighting state.
- The visible edge overlay still exists, but only when `RenderConfig.debug.debug_overlay` is enabled.
