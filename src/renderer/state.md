# state

## Role

- Define renderer-owned runtime state

## Notes

- `RenderEnvironmentState` still owns the mutable sun/weather/climate values.
- `RendererBackend` now owns:
  - camera/environment/sun-shadow uniform buffers and bind groups
  - block-texture bind group/layout
  - UI atlas texture bind group/layout
  - main depth resources
  - shadow-map resources
  - sun overlay, shadow depth, terrain, dynamic, UI sprite, and debug pipelines
