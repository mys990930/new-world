# pipeline

## Role

- Track renderer pipeline metadata independently from the live `wgpu::RenderPipeline` handles

## Owned Data

- sun overlay pipeline metadata
- terrain opaque pipeline metadata
- dynamic opaque pipeline metadata
- shadow depth pipeline metadata
- optional debug overlay pipeline metadata

## Notes

- The live backend now owns five logical passes:
  - sun overlay
  - shadow depth
  - terrain opaque
  - dynamic opaque
  - optional debug overlay
