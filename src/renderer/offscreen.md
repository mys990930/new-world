# offscreen

## Role

- Render renderer-ready CPU meshes into an offscreen RGBA image without a live window surface

## Responsibilities

- create a headless `wgpu` device/queue for debug rendering
- decode renderer texture-array DTOs into GPU texture resources
- upload camera, environment, and disabled shadow uniforms
- draw terrain meshes into an offscreen color target
- copy the rendered image back to CPU memory
- write the resulting image to PNG when requested

## Non-Responsibilities

- world generation
- chunk meshing
- visible chunk calculation
- gameplay camera control
- live window present

## Inputs

- `OffscreenRenderRequest`
  - output image size
  - renderer camera state
  - renderer texture array source
  - renderer environment
  - render-ready CPU meshes
  - optional clear-color override

## Outputs

- `OffscreenRenderOutput`
  - `width`
  - `height`
  - tightly packed RGBA pixels
  - draw-call count

## Public Interface

```rust
render_offscreen(request: OffscreenRenderRequest) -> Result<OffscreenRenderOutput, OffscreenRenderError>
write_offscreen_png(path, image: &OffscreenRenderOutput) -> Result<(), OffscreenRenderError>
```

## Invariants

1. Offscreen rendering consumes renderer-ready DTOs only.
2. Offscreen rendering does not require or create a window-backed surface.
3. The texture-array contract matches the live renderer contract: same tile size, contiguous layers, `Rgba8UnormSrgb`.
4. The current offscreen path is terrain-only debug rendering; it does not own world meshing or gameplay overlays.

## Related Modules

- `renderer.md`
- `camera.rs`
- `surface.rs`
- `texture.rs`
- `upload.rs`

## Notes

- This module exists so debug binaries can inspect generated terrain with the real terrain shader without booting the full app loop.
- The current implementation binds a disabled shadow resource so terrain shading matches the live renderer contract closely enough for visual inspection.
