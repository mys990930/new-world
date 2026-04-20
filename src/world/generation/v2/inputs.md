# inputs

## Role

- hold the assembled atlas, skeleton, region-classification, and meso guide inputs for V2 chunk generation
- carry the pre-realization-field raw inputs that later realization, corridor, prototype, and meso stages consume

## Current Types

- `ChunkGenerationV2Inputs`
- `ChunkGenerationV2Scaffold`
- `V2ScaffoldStage`

## Notes

- `ChunkGenerationV2Inputs` still stops at the pre-realization-field checkpoint
- `ChunkGenerationV2Inputs` now also carries the prebuilt `MesoGuideMap` so later chunk-local deformation does not need to regenerate atlas-owned guides
- `ChunkGenerationV2Scaffold` now carries both a solved `ChunkRealizationFieldPatch` and a `ChunkCorridorWindow` forward so prototype can consume explicit pre-meso control/state without rebuilding either stage
- `V2ScaffoldStage` now includes `RealizationFieldReady` between `RegionClassificationReady` and `CorridorWindowReady`
- the next stage boundaries are the realization-field solve in `v2/realization_field.md`, the corridor solve in `v2/corridors.md`, the base-heightfield prototype solve in `v2/prototype.md`, and the post-prototype meso deformation pass in `v2/meso_apply.md`
