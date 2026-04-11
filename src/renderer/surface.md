# surface

## Role

- Own renderer surface/backend initialization and resize handling

## Responsibilities

- Define the `RenderSurfaceTarget` abstraction
- Support both stub and live window-backed targets
- Create `wgpu::Instance`, `Surface`, `Adapter`, `Device`, and `Queue`
- Create and recreate the main depth texture on resize
- Create the shadow-map texture and comparison sampler
- Create the camera, environment, and sun-shadow uniform buffers and bind groups
- Create the block-texture bind group layout and GPU texture-array resources
- Configure and reconfigure the surface
- Bridge renderer bootstrap time and post-`resumed()` live attach time
- Build the sun overlay, shadow depth, terrain, dynamic cube, and debug edge pipelines

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
- Shadow-map resolution is driven by render quality rather than surface size

## Invariants

- A zero-sized surface is not configured
- Real surface configure/present only happens when a live backend exists
- Terrain and dynamic passes share the same binding contract so the frame encoder can switch pipelines without rebinding camera/environment/texture/shadow ownership data

## Related Modules

- `state.rs`
- `camera.rs`
- `frame.rs`

## Notes

- The backend now creates the sun overlay pipeline, shadow depth pipeline, terrain pipeline, dynamic cube pipeline, and debug edge pipeline during initialization.
- Shadow maps currently use a single `Depth32Float` texture plus comparison sampler.
- Medium quality uses a smaller hard-sun map than high quality.
- The environment uniform test helper now follows the current default environment preset instead of pinning a legacy sunset-only baseline.
