# surface

## Role

- Own renderer surface/backend initialization and resize handling

## Responsibilities

- Define the `RenderSurfaceTarget` abstraction
- Support both stub and live window-backed targets
- Create `wgpu::Instance`, `Surface`, `Adapter`, `Device`, and `Queue`
- Create and recreate the depth texture on resize
- Create the camera uniform buffer and bind group
- Create the environment uniform buffer and bind group
- Create the block-texture bind group layout and GPU texture-array resources
- Configure and reconfigure the surface
- Bridge renderer bootstrap time and post-`resumed()` live attach time
- Build the terrain, dynamic cube, and debug edge pipelines used by the current prototype

## Non-Responsibilities

- Creating frame DTOs
- Encoding per-frame draw calls
- Interpreting gameplay state

## Public Interface

```rust
Renderer::new(target: &impl RenderSurfaceTarget, config: RenderConfig) -> Result<Renderer, RenderInitError>
Renderer::attach_window_surface(window: Arc<Window>) -> Result<(), RenderInitError>
Renderer::resize(width: u32, height: u32) -> Result<(), RenderSurfaceError>
```

## State Rules

- A renderer created from a stub target may have no live `RendererBackend`
- Attaching a live window creates the real `wgpu` backend
- Resize updates `SurfaceState`, camera aspect cache, and live surface configuration together

## Invariants

- A zero-sized surface is not configured
- Real surface configure/present only happens when a live backend exists
- Terrain and dynamic passes share the same binding contract so the frame encoder can switch pipelines without rebinding ownership data

## Related Modules

- `state.rs`
- `camera.rs`
- `frame.rs`

## Notes

- The current backend creates the terrain pipeline, dynamic cube pipeline, debug edge pipeline, camera uniform buffer, environment uniform buffer, and block-texture resources during initialization.
- Depth resources are recreated together with surface resize.
- When multiple sRGB surface formats are available, the current prototype prefers `Rgba8UnormSrgb` over `Bgra8UnormSrgb` so warm sunset color tuning reads predictably during bring-up.
- The current environment uniform is the renderer-side expansion point for future day-night, weather, and climate systems.
