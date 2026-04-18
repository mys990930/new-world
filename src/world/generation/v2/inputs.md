# inputs

## Role

- hold the assembled atlas, skeleton, region-classification, and meso guide inputs for V2 chunk generation
- carry the early corridor handoff checkpoint that later prototype and meso stages consume

## Current Types

- `ChunkGenerationV2Inputs`
- `ChunkGenerationV2Scaffold`
- `V2ScaffoldStage`

## Notes

- this module now stops at the corridor-window-ready checkpoint
- `ChunkGenerationV2Inputs` now also carries the prebuilt `MesoGuideMap` so later chunk-local deformation does not need to regenerate atlas-owned guides
- `ChunkGenerationV2Scaffold` carries the corridor window forward so prototype can consume it explicitly
- the next stage boundaries are the base-heightfield prototype solve in `v2/prototype.md` and the post-prototype meso deformation pass in `v2/meso_apply.md`
