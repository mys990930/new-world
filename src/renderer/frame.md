# frame

## Role

- Encode renderer frame passes and perform submit/present

## Responsibilities

- Update frame stats
- Update GPU camera state
- Upload the camera uniform
- Upload the environment uniform
- Acquire the surface texture
- Clear and use the depth buffer
- Bind the block texture array
- Draw uploaded chunk meshes for the current `visible_chunks`
- Expand `cube_instances` into a white cube mesh with per-face normals
- Draw the terrain pass
- Draw the dynamic cube pass
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
3. Resolve the clear color from the current renderer environment unless the app overrides it.
4. If there is no live backend, return stats only.
5. Upload the camera uniform.
6. Upload the environment uniform derived from the current quality preset and environment state.
7. Acquire the surface texture.
8. Run the main pass to clear and draw uploaded terrain chunk meshes.
9. Switch pipeline inside the same pass and draw dynamic cube instances.
10. If `debug_overlay` is enabled, compute visible edges and draw the overlay pass.
11. Submit and present.

## Invariants

- Renderer only sees render-ready DTOs such as `RenderCubeInstance`.
- If there is no live backend or the surface is not configured, nothing is presented.
- Acquire failures are returned as recoverable `RenderSurfaceError` values.
- Terrain and dynamic cubes share camera / environment / texture bind groups but not necessarily the same shader logic.

## Related Modules

- `camera.rs`
- `surface.rs`
- `state.rs`

## Notes

- The current implementation first draws uploaded chunk meshes with the terrain shader, then overlays dynamic cube instances such as the player body and the thin ground shadow slab with the dynamic shader.
- The dynamic cube path still uses the built-in white texture layer plus tint color, but it now participates in the same sunset environment and fog model as terrain.
- The visible edge overlay still exists, but only when `RenderConfig.debug.debug_overlay` is enabled.
