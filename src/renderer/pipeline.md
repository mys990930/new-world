# pipeline

## Role

- Track renderer pipeline metadata independently from the live `wgpu::RenderPipeline` handles

## Responsibilities

- Describe which logical pipelines the renderer expects
- Mirror sample-count and depth policy at the metadata layer
- Track rebuild generations when the surface is reconfigured

## Non-Responsibilities

- CPU mesh generation
- camera math
- frame-loop ownership
- chunk visibility decisions
- live shader / bind-group creation

## Owned Data

- terrain opaque pipeline metadata
- dynamic opaque pipeline metadata
- optional debug overlay pipeline metadata

## Inputs

- `RenderConfig`
- current `SurfaceState`

## Outputs

- `PipelineSet`
- rebuild-generation hints for renderer runtime code

## Processing Flow

1. Build metadata for the terrain opaque pipeline.
2. Build metadata for the dynamic opaque pipeline.
3. Optionally build metadata for the debug overlay pipeline.
4. Bump rebuild generations whenever the surface is reconfigured.

## Invariants

- metadata must stay aligned with the logical draw structure used in `frame.rs`
- debug overlay metadata only exists when the debug flag is enabled
- pipeline metadata does not own live GPU resources

## Related Modules

- `config.rs`
- `state.rs`
- `surface.rs`
- `frame.rs`

## Notes

- The live `wgpu::RenderPipeline` handles are created in `surface.rs` and stored in `RendererBackend`.
- `pipeline.rs` remains useful as the stable description of what kinds of passes the renderer believes exist, even while the live backend is rebuilt.
