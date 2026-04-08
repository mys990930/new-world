# surface

## Role

- Own renderer surface/backend initialization and resize handling

## Responsibilities

- Define the `RenderSurfaceTarget` abstraction
- Support both stub and live window-backed targets
- Create `wgpu::Instance`, `Surface`, `Adapter`, `Device`, and `Queue`
- Create and recreate the depth texture on resize
- Configure and reconfigure the surface
- Bridge renderer bootstrap time and post-`resumed()` live attach time
- Build the fill and edge overlay pipelines used by the current cube prototype

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
- The current filled-cube pipeline leaves face culling disabled so the depth-tested prototype cube is not sensitive to winding mistakes during early renderer bring-up

## Related Modules

- `state.rs`
- `camera.rs`
- `frame.rs`

## Notes

- The current backend creates the player-cube fill pipeline, edge overlay pipeline, and camera uniform buffer during initialization
- Depth resources are recreated together with surface resize
- When multiple sRGB surface formats are available, the current prototype prefers `Rgba8UnormSrgb` over `Bgra8UnormSrgb` so debug face colors read more predictably during renderer bring-up
