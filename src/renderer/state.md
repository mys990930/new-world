# state

## Role

- Define renderer-owned runtime state

## Responsibilities

- Store top-level `Renderer` runtime state
- Store CPU-side render world bookkeeping
- Store the optional live GPU backend
- Store last-frame stats and frame index

## Owned Data

- `RenderConfig`
- `SurfaceState`
- `PipelineSet`
- `RenderWorld`
- `CameraGpuState`
- `Option<RendererBackend>`
- `RenderStats`
- `frame_index`

## Non-Responsibilities

- Creating windows
- Creating render DTOs
- Owning gameplay state

## Invariants

- `backend == None` can still represent a valid stub renderer state
- Live GPU resources only exist inside `RendererBackend`
- `RenderWorld` is renderer cache/meta state, not world source-of-truth data

## Related Modules

- `surface.rs`
- `frame.rs`
- `camera.rs`
- `upload.rs`

## Notes

- `RendererBackend` currently owns the `wgpu::Surface`, `Device`, `Queue`, camera/light uniform buffers and bind groups, depth resources, and the cube pipelines.
