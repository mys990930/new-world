# inputs

## Role

- hold the assembled atlas, skeleton, region-classification, and meso guide inputs for V2 chunk generation
- carry the pre-realization-field raw inputs that later realization, corridor, prototype, and meso stages consume

## Current Types

- `ChunkGenerationV2Inputs`
- `ChunkGenerationV2Scaffold`
- `V2ScaffoldStage`

## Notes

- this module now stops at the pre-realization-field checkpoint
- `ChunkGenerationV2Inputs` now also carries the prebuilt `MesoGuideMap` so later chunk-local deformation does not need to regenerate atlas-owned guides
- the current `ChunkGenerationV2Scaffold` still carries a corridor window forward as temporary runtime scaffolding, even though the longer-term design now inserts realization field between raw inputs and prototype
- the next stage boundaries are the realization-field solve in `v2/realization_field.md`, the corridor solve in `v2/corridors.md`, the base-heightfield prototype solve in `v2/prototype.md`, and the post-prototype meso deformation pass in `v2/meso_apply.md`
