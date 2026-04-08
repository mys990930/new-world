## renderer

### Role

- Own the GPU / surface / draw / present boundary
- Execute render-ready DTOs produced by the app bridge

### Responsibilities

- Renderer bootstrap and GPU context setup
- Stub surface state and live window surface attach
- Surface configure / resize handling
- Render pipeline, shader, camera uniform, and light uniform management
- Depth buffer creation and recreation
- CPU render DTO -> GPU draw command conversion
- Submit / present / recoverable render error propagation

### Non-Responsibilities

- Meshing algorithms
- Visible chunk calculation
- World source-of-truth ownership
- Gameplay input interpretation
- Main loop orchestration

### Data

- `RenderConfig`
- `Renderer`
- `SurfaceState`
- `PipelineSet`
- `RenderWorld`
- `CameraGpuState`
- `RenderCubeInstance`
- `RenderStats`

### Use Cases

- initialization
  - create a renderer from a stub target or a live window target
- live surface attach
  - after `resumed()`, receive the real window and create the `wgpu` backend
- resize
  - update surface config and projection-related runtime state
- frame render
  - accept `RenderFrameInput`, update camera uniforms, draw dynamic cubes, and present

### Public Interface

```rust
Renderer::new(target: &impl RenderSurfaceTarget, config: RenderConfig) -> Result<Renderer, RenderInitError>
Renderer::attach_window_surface(window: Arc<Window>) -> Result<(), RenderInitError>
Renderer::resize(width: u32, height: u32) -> Result<(), RenderSurfaceError>

Renderer::apply_upload(request: RenderUploadRequest) -> Result<(), RenderUploadError>
Renderer::remove_chunk_mesh(coord: ChunkCoord)

Renderer::render(frame: RenderFrameInput<'_>) -> Result<RenderStats, RenderError>

struct RenderFrameInput<'a> {
    camera: &'a RenderCameraState,
    visible_chunks: &'a [ChunkCoord],
    cube_instances: &'a [RenderCubeInstance],
    clear_color_override: Option<[f32; 4]>,
}
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

### Current Implementation Notes

- The current vertical slice renders a single player cube through the minimal dynamic object path.
- The player cube now uses a white base color plus a fixed directional light, with optional edge overlay and an orthographic quarter-view camera.
- The player cube path uses depth test/write so the cube reads as a solid volume.
- The chunk upload path is still CPU-side bookkeeping only; real chunk drawing comes later.

### Submodules

- mod.rs: public facade, re-export
- config.rs: `RenderConfig` and renderer policy settings
- state.rs: `Renderer`, `RenderWorld`, optional `RendererBackend`
- surface.rs: device / queue / surface initialization, live surface attach, resize management, light buffer setup
- pipeline.rs: pipeline metadata
- camera.rs: `RenderCameraState` -> `CameraGpuState` / `CameraUniform`
- upload.rs: mesh/upload DTOs and shared vertex layout definitions
- frame.rs: frame render pass encode, dynamic cube draw, submit, present
