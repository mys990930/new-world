# atlas_realization_chunk_topdown_preview

## Role

- Render one seed-driven PNG that compares the same top-down `xz` footprint at three generation stages:
  - atlas-owned biome / region classification
  - generation-side realization control field
  - final realized chunk top surface
- Keep all three stage images at the same world-block scale so boundaries can be compared directly.

## Inputs

- `seed`
- preview center in chunk coordinates via `--center-x` / `--center-z`
- `--chunk-x` / `--chunk-z` aliases for direct chunk targeting
- horizontal chunk `--radius`
- vertical final-chunk scan bounds via `--min-y-chunk` / `--max-y-chunk`
- image scale via `--pixels-per-block`
- output PNG path

## Outputs

- a single PNG under `target/atlas-realization-chunk-topdown-preview/` by default
- the image contains three left-to-right panels:
  - `ATLAS BIOME`: region-classification biome palette using the atlas/region preview style
  - `REALIZATION`: composite continuous control color
  - `FINAL CHUNK`: exact topmost realized non-air block color from the generated chunks
- each panel includes a compact in-image legend that names the primary color channels without taking focus away from the top-down comparison
- stdout prints the scale, footprint, sampled atlas bounds, realization channel stats, and top visible final blocks

## Current Flow

1. Build a per-preview generation input cache keyed by generation atlas area, matching `chunk_preview`.
2. For the atlas panel, sample the cached `RegionClassMap` at every world block in the requested footprint.
3. For the realization panel, build `ChunkRealizationFieldPatch` from the same cached inputs and sample the control field at the same world block positions.
4. For the final chunk panel, build the current generation voxelization plan from the same cached inputs, generate the requested vertical chunk stack, insert it into `WorldCore`, and sample exact top-down columns.
5. Draw all stage panels side-by-side with the same `pixels-per-block` scale and add compact legends inside the final combined image.

## Color Semantics

- Atlas biome panel:
  - base color is `BiomeFamily`
  - coast, hydrology, terrain form, relief, and elevation modify the base color
  - dark lines mark chunk boundaries, atlas boundaries, and region transitions
- Realization panel:
  - red channel tracks uplift / ridge / terrace pressure
  - green channel tracks flatness / lowland continuity
  - blue channel tracks wetness / corridor pressure
  - brightness carries relief / uplift strength
  - dark lines mark chunk boundaries, atlas boundaries, and stronger control transitions
- Final chunk panel:
  - base color comes from the top visible block material and block tint
  - brightness tracks relative surface `y`
  - dark borders mark block and surface-height changes

## Notes

- This binary intentionally does not invoke `realization_field_preview`; it builds the realization panel from `build_chunk_realization_field_patch(...)` directly.
- The atlas, realization, and chunk panels all share the same world footprint and pixel scale. Increasing `--radius` or `--pixels-per-block` can produce a very large image by design.
- The composed image keeps the three top-down panels as the primary content; legends are intentionally short and only summarize color semantics plus the most common sampled classes.
