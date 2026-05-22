# pipeline

## Role

- Track renderer pipeline metadata independently from the live `wgpu::RenderPipeline` handles

## Owned Data

- sun overlay pipeline metadata
- terrain opaque pipeline metadata
- terrain fade pipeline metadata may remain implicit while it shares the terrain shader/layout
- translucent water pipeline metadata may remain implicit while it shares the terrain shader/layout
- dynamic opaque pipeline metadata
- shadow depth pipeline metadata
- optional debug overlay pipeline metadata

## Notes

- The live backend now owns the following logical scene/UI passes:
  - sun overlay
  - shadow depth
  - terrain opaque
  - terrain fade for player-covering occlusion blocks
  - dynamic opaque
  - translucent water
  - UI sprite
  - optional debug overlay
