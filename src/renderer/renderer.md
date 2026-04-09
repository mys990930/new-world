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
4. Block textures are uploaded as a same-size `texture_2d_array`, and mesh vertices address them by `texture_layer`.
5. Terrain and dynamic cubes now use different shader modules but share the same camera / environment / texture binding contract.
6. The current environment state lives inside the renderer so future day-night, weather, and climate systems can drive it without moving lighting policy into the world or app layers.

### Current Implementation Notes

- The current vertical slice renders uploaded chunk terrain plus dynamic cube instances through separate terrain and dynamic pipelines.
- Block textures are loaded from PNG files into an `Rgba8UnormSrgb` texture array with nearest filtering.
- Dynamic cube instances currently include the white player cube, a thin dark ground shadow slab, and a thin yellow hovered-face highlight slab.
- The default environment is a fixed sunset quarter-view preset with fog, warm horizon tint, and readability boosts for top faces and silhouettes.
- Render quality is organized as low / medium / high presets. The current implementation uses them to gate fog, color grading, climate tint, and weather tint while reserving shadow quality for a later pass.
- Both chunk meshes and dynamic cubes use depth test/write so the plane/cube layering reads as solid volume.
- The renderer caches CPU chunk mesh payloads and builds GPU vertex/index buffers when a live backend exists.
- Texture layer `0` is reserved for the built-in white tile used by debug cubes and tint-only draws.

### Submodules

- `mod.rs`: public facade and re-exports
- `config.rs`: `RenderConfig`, quality presets, and fixed environment settings
- `state.rs`: `Renderer`, `RenderEnvironmentState`, `RenderWorld`, optional `RendererBackend`
- `surface.rs`: device / queue / surface initialization, live surface attach, resize management, environment uniform setup, pipeline creation
- `pipeline.rs`: renderer pipeline metadata and rebuild generations
- `camera.rs`: `RenderCameraState` -> `CameraGpuState` / `CameraUniform`
- `texture.rs`: block texture source DTOs, PNG decode/validation, texture-array upload helpers
- `upload.rs`: mesh/upload DTOs and shared vertex layout definitions
- `frame.rs`: frame render pass encode, terrain draw, dynamic cube draw, submit, present
