# region_topdown_preview

## Role

- render a seed-driven top-down PNG from atlas-owned region classification instead of realized block materials
- color the image by biome family while still preserving chunk-scale region identity cues through region-aware tinting and transition lines
- sit between atlas biome preview and exact `chunk_topdown_preview` for debugging future realization-field transitions

## Inputs

- `seed`
- preview center in chunk coordinates via `--center-x` / `--center-z`
- `--chunk-x` / `--chunk-z` aliases for direct chunk targeting
- horizontal chunk `--radius`
- `--blocks-per-pixel <u32>` sample scale
- output PNG path

## Outputs

- a PNG image where each pixel represents one sampled world-space region cell footprint
- a stdout summary with:
  - chunk footprint
  - scale and sampled atlas-cell range
  - dominant biome, archetype, terrain-form, and hydrology counts

## Current Flow

1. Build a seed-driven atlas field window that covers the requested chunk footprint plus one-cell padding.
2. Resolve atlas structure and launch-safe region classes for that same window.
3. Sample `sample_region_classes(...)` across the requested chunk window in world space.
4. Convert each sampled region cell into a biome-family palette color with region-aware tinting.
5. Darken chunk lines, atlas-cell lines, and region transitions so future realization boundaries are easier to spot.
6. Save the PNG and print the sampled identity summary.

## Scale Semantics

- default scale is `--blocks-per-pixel 8`
- with the current `32 x 32` chunk edge, that means one pixel covers one quarter-chunk footprint
- equivalently:
  - `8 x 8` world blocks per pixel
  - `4 x 4` pixels per chunk
- this is intentionally denser than atlas debug images, but coarser than exact realized block-column previews
- changing `--blocks-per-pixel` changes both sampling density and final image dimensions

## Identity Notes

- this tool does not read realized block materials or scan vertical chunk columns
- the image is driven by atlas-owned `BiomeFamily`, `TerrainFormFamily`, `HydrologyContext`, and `RegionArchetype` classification
- because current public `sample_region_classes(...)` resolves by world-space sample position into the containing atlas region cell, the preview is useful for inspecting where region ownership changes across a chunk window before full realization is active
