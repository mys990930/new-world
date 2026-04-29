# inputs

## Role

- hold the assembled atlas, skeleton, region-classification, and meso guide inputs for chunk generation
- carry the pre-realization-field raw inputs that later realization, corridor, prototype, and meso stages consume

## Current Types

- `ChunkGenerationInputs`
- `ChunkGenerationInputCache`
- `ChunkGenerationScaffold`
- `GenerationScaffoldStage`

## Notes

- `ChunkGenerationInputs` still stops at the pre-realization-field checkpoint
- `ChunkGenerationInputs` carries the world seed alongside the assembled maps so deterministic
  downstream boundary warps can stay generation-order-independent without rebuilding inputs from
  `WorldMeta`
- `ChunkGenerationInputs` now also carries the prebuilt `MesoGuideMap` so later chunk-local deformation does not need to regenerate atlas-owned guides
- `ChunkGenerationInputCache` reuses deterministic atlas / structure / region / meso bundles for chunks that share the same generation atlas area; callers still receive a per-chunk `ChunkGenerationInputs` with the requested `ChunkCoord`
- `chunk_generation_input_area(...)` exposes the exact atlas footprint used for this reuse decision so tools can batch or cache work without changing generation semantics
- `ChunkGenerationScaffold` now carries both a solved `ChunkRealizationFieldPatch` and a `ChunkCorridorWindow` forward so prototype can consume explicit pre-meso control/state without rebuilding either stage
- `GenerationScaffoldStage` now includes `RealizationFieldReady` between `RegionClassificationReady` and `CorridorWindowReady`
- the next stage boundaries are the realization-field solve in `realization_field.md`, the corridor solve in `corridors.md`, the base-heightfield prototype solve in `prototype.md`, and the post-prototype meso deformation pass in `meso_apply.md`
