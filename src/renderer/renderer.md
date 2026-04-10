## renderer

### Role

- Own the GPU / surface / shader / draw / present boundary
- Execute render-ready DTOs produced by the app bridge

### Responsibilities

- Renderer bootstrap and GPU context setup
- Stub surface state and live window surface attach
- Surface configure / resize handling
- Render pipeline, shader, camera uniform, and environment uniform management
- Block texture array decoding policy and GPU bind-group management
- Consume renderer material kinds and run material-aware shading
- Depth buffer creation and recreation
- CPU render DTO -> GPU draw command conversion
- Submit / present / recoverable render error propagation
- Maintain renderer-owned quality presets and a fixed environment state until gameplay systems drive them

### Non-Responsibilities

- Meshing algorithms
- Visible chunk calculation
- World source-of-truth ownership
- Gameplay input interpretation
- Main loop orchestration

### Data

- `RenderConfig`
- `RenderQualityConfig`
- `RenderEnvironment`
- `RenderEnvironmentState`
- `RenderMaterialKind`
- `Renderer`
- `SurfaceState`
- `PipelineSet`
- `RenderWorld`
- `CameraGpuState`
- `RenderTextureArraySource`
- `RenderCubeInstance`
- `RenderStats`

### Use Cases

- initialization
  - create a renderer from a stub target or a live window target
- live surface attach
  - after `resumed()`, receive the real window and create the `wgpu` backend
- resize
  - update surface config and projection-related runtime state
- environment tuning
  - swap the current sunset / weather / climate values without changing app-facing DTO shape
- frame render
  - accept `RenderFrameInput`, update camera and environment uniforms, draw terrain chunks plus dynamic cubes, and present

### Public Interface

```rust
Renderer::new(target: &impl RenderSurfaceTarget, config: RenderConfig) -> Result<Renderer, RenderInitError>
Renderer::attach_window_surface(window: Arc<Window>) -> Result<(), RenderInitError>
Renderer::resize(width: u32, height: u32) -> Result<(), RenderSurfaceError>

Renderer::set_block_textures(source: RenderTextureArraySource) -> Result<(), RenderTextureError>
Renderer::set_environment(environment: RenderEnvironment)
Renderer::apply_upload(request: RenderUploadRequest) -> Result<(), RenderUploadError>
Renderer::remove_chunk_mesh(coord: ChunkCoord)

Renderer::render(frame: RenderFrameInput<'_>) -> Result<RenderStats, RenderError>
```

### Dependencies

- `wgpu`
- platform window / surface creation input
- render DTOs produced by `app::bridge`

NOT:

- ECS internal resource layout
- World internal data layout
- App runner ownership

### Invariants

1. Renderer only consumes render-ready DTOs.
2. During bootstrap, a stub renderer may exist without a live backend, but real draw/present only happens after a live backend is attached.
3. GPU resource creation and destruction happen only inside the renderer.
4. Block textures are uploaded as a same-size `texture_2d_array`, and mesh vertices address them by `texture_layer`.
5. Mesh vertices also carry `material_kind`, but the renderer only interprets renderer-side shading enums and never queries world block definitions directly.
6. Terrain and dynamic cubes use different shader modules while sharing the same camera / environment / texture binding contract.

### Current Implementation Notes

- Uploaded chunk terrain now includes material classification from the world registry.
- The terrain shader preserves more raw texture detail before fog and color grading, so dirt/grass/stone read more clearly in quarter view.
- Dynamic cube instances now distinguish actor, shadow, and highlight behavior through `RenderMaterialKind`.
- The default environment is still a fixed sunset quarter-view preset with low/medium/high quality flags for future extension.
