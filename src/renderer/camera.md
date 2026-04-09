# camera

## Role

- Convert render camera DTOs into GPU-ready camera state and uniform data

## Responsibilities

- Define `RenderCameraState`
- Define `RenderProjectionMode` and `RenderViewBasis`
- Compute `view`, `projection`, and `view_projection`
- Cache aspect ratio on resize
- Build `CameraUniform`

## Non-Responsibilities

- Deciding gameplay camera behavior
- Calculating follow, deadzone, recenter, or movement bias
- Encoding draw passes

## Inputs

- `RenderCameraState`
- `CameraProjectionConfig`
- Surface width and height

## Outputs

- `CameraGpuState`
- `CameraUniform`

## Boundary Rules

- Renderer consumes final render camera pose only
- Gameplay camera rules stay in ECS/app
- CPU-side math is stored row-major, and the raw bytes are uploaded directly so WGSL's column-major matrix interpretation sees the intended transform

## Invariants

- `RenderViewBasis.right` and `RenderViewBasis.up` describe screen axes directly
- `RenderViewBasis.forward` points from the camera toward the target
- Renderer uses the explicit basis as-is and does not reinterpret gameplay meaning from it
- `view_from_basis` expects a normalized, internally consistent basis

## Related Modules

- `frame.rs`
- `surface.rs`
- `config.rs`

## Notes

- Renderer expects an orthographic quarter-view pose through an explicit basis override
- Loose follow, deadzone, movement bias, and smooth recenter are resolved upstream in ECS/app before the renderer sees the camera DTO
- `CameraUniform` now includes the eye position in addition to the view-projection matrix so fog and silhouette lighting can be computed fully inside the renderer
- Camera uniform upload must not apply an extra transpose; doing so skews the screen axes and collapses the cube into the wrong silhouette
