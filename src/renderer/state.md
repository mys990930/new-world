# state

## Role

- Define renderer-owned runtime state

## Responsibilities

- Store top-level `Renderer` runtime state
- Store CPU-side render world bookkeeping
- Store CPU-side block texture payloads that can survive stub/live backend transitions
- Store the mutable renderer environment state that future day-night / weather systems can drive
- Store the optional live GPU backend
- Store last-frame stats and frame index

## Owned Data

- `RenderConfig`
- `SurfaceState`
- `PipelineSet`
- `RenderWorld`
- `RenderEnvironmentState`
- `CameraGpuState`
- `BlockTextureSet`
- `Option<RendererBackend>`
- `RenderStats`
- `frame_index`

## Non-Responsibilities

- Creating windows
- Creating render DTOs
- Owning gameplay state

## Invariants

- `backend == None` can still represent a valid stub renderer state
- live GPU resources only exist inside `RendererBackend`
- `RenderWorld` is renderer cache/meta state, not world source-of-truth data
- CPU-side block textures may exist before a live backend does, and are uploaded when the backend becomes available
- `RenderEnvironmentState` is renderer-owned runtime tuning data, not a world simulation authority

## Related Modules

- `surface.rs`
- `frame.rs`
- `camera.rs`
- `upload.rs`

## Notes

- `RenderEnvironmentState` stores the current environment plus a generation counter so future systems can change sunset, weather, or climate values without rebuilding the renderer.
- `RendererBackend` now owns the `wgpu::Surface`, `Device`, `Queue`, camera/environment uniform buffers and bind groups, block-texture bind group/layout, depth resources, and the terrain / dynamic / debug pipelines.
