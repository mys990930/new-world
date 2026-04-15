## renderer

### Role

- Own the GPU / surface / shader / draw / present boundary
- Execute render-ready DTOs produced by the app bridge

### Responsibilities

- Renderer bootstrap and GPU context setup
- Stub surface state and live window surface attach
- Surface configure / resize handling
- Render pipeline, shader, camera uniform, environment uniform, and sun-shadow uniform management
- Block texture array decoding policy and GPU bind-group management
- Visible sun overlay and directional shadow-map rendering
- Consume renderer material kinds and run material-aware shading
- Depth buffer and shadow-map creation/recreation
- CPU render DTO -> GPU draw command conversion
- screen-space UI sprite draw submission
- Submit / present / recoverable render error propagation
- Offscreen terrain preview rendering for debug binaries
- Maintain renderer-owned quality presets and a fixed environment state until gameplay systems drive them
- Consume render-facing octant / pose DTOs for future moving voxel entities without owning gameplay-facing yaw rules

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
- `RenderUiSprite`
- `RenderUiTextureSource`
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
  - accept `RenderFrameInput`, update scene uniforms when needed, draw the scene passes, draw the UI overlay pass, and present
- offscreen preview render
  - accept renderer-ready meshes and render them into a PNG-friendly RGBA image without a live surface

### Public Interface

```rust
Renderer::new(target: &impl RenderSurfaceTarget, config: RenderConfig) -> Result<Renderer, RenderInitError>
Renderer::attach_window_surface(window: Arc<Window>) -> Result<(), RenderInitError>
Renderer::resize(width: u32, height: u32) -> Result<(), RenderSurfaceError>

Renderer::set_block_textures(source: RenderTextureArraySource) -> Result<(), RenderTextureError>
Renderer::set_ui_texture(source: RenderUiTextureSource) -> Result<(), RenderUiTextureError>
Renderer::set_environment(environment: RenderEnvironment)
Renderer::apply_upload(request: RenderUploadRequest) -> Result<(), RenderUploadError>
Renderer::remove_chunk_mesh(coord: ChunkCoord)
Renderer::clear_chunk_meshes()

Renderer::render(frame: RenderFrameInput<'_>) -> Result<RenderStats, RenderError>

render_offscreen(request: OffscreenRenderRequest) -> Result<OffscreenRenderOutput, OffscreenRenderError>
write_offscreen_png(path, image: &OffscreenRenderOutput) -> Result<(), OffscreenRenderError>
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
6. The visible sun and shadow-map logic are renderer-owned visualizations of the current environment state, not gameplay-owned world objects.
7. Future moving voxel-entity rendering should consume bridge-produced octant / pose data and must not infer gameplay-facing direction from velocity or input on its own.
8. Screen-space UI sprites stay renderer-local DTOs and do not expose ECS/world ownership.

### Current Implementation Notes

- Uploaded chunk terrain includes material classification from the world registry.
- The terrain shader preserves more raw texture detail before atmosphere/fog grading, so dirt/grass/stone read more clearly in quarter view.
- The terrain shader now consumes world-provided top-face contour edges, so layer breaks read on actual height transitions instead of every block border.
- A visible sun overlay is now drawn from the current `sun_direction`.
- Terrain and dynamic cubes now sample a directional shadow map derived from visible geometry bounds.
- The current shadow solution is a single hard-sun shadow map sized by quality tier.
- Dynamic cube instances distinguish actor, shadow, and highlight behavior through `RenderMaterialKind`.
- Screen-space app UI currently enters as `RenderUiSprite` and is rendered in a dedicated overlay pass with no camera/world dependency.
- The default environment is now a fixed sunset quarter-view preset tuned to preserve chunk contrast while keeping a light amount of atmospheric fog, and medium/high quality still enable the shadow-map path.
- The renderer can already consume arbitrary time/weather/climate values through `RenderEnvironment`, but the main app loop is not yet driving a live day-night/weather simulation.
- Offscreen preview rendering currently reuses the terrain shader and texture-array contract, but skips live-surface present and dynamic gameplay overlays.
- Fixed block terrain keeps its chunk-mesh reuse advantages even when moving entities are present; dynamic entity cost is additive rather than replacing the static-terrain path.
- For future animated voxel creatures, precreated data is still useful: the recommended direction is to select among `(pose_id, facing_octant)` render assets or part poses, rather than treating every animation frame as a fully procedural free-rotation mesh build.
- Water triangles are now renderer-split into a translucent terrain partition so semi-transparent water can render after opaque terrain without changing the app bridge DTO shape.
- Exposed water surface height remains world-owned geometry; the renderer only shades and blends the lowered mesh it receives.
