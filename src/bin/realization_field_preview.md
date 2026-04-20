# realization_field_preview

## Role

- render a seed-driven top-down PNG for the generation-side realization / prototype-control field
- sit between `region_topdown_preview` and later exact realized chunk previews while V2 realization is still landing
- keep the CLI and summary style close to the other debug preview binaries under `src/bin`

## Inputs

- `seed`
- preview center in chunk coordinates via `--center-x` / `--center-z`
- `--chunk-x` / `--chunk-z` aliases for direct chunk targeting
- horizontal chunk `--radius`
- `--blocks-per-pixel <u32>` sample scale
- `--mode <name>`
- output PNG path

## Outputs

- a PNG image where each pixel represents one sampled realization/control value over an `xz` footprint
- a stdout summary with:
  - chunk footprint
  - sample scale and sampled atlas-cell range
  - per-channel min/max/average for the control preview
  - dominant biome, archetype, terrain-form, and hydrology counts at the sampled positions

## Current Flow

1. For each chunk in the requested preview window, build the public V2 scaffold via `build_chunk_v2_scaffold(...)`.
2. Sample one control point per preview pixel inside that chunk footprint.
3. Sample the chunk's public `ChunkRealizationFieldPatch` in world space to obtain the continuous prototype-control vector actually used by prototype.
4. Normalize each preview channel against the current window's sampled range so subtle intra-window variation stays visible even inside large single-archetype regions.
5. Color either a composite broad-shape view or a named single-channel mode.
6. Darken chunk lines, atlas-cell lines, and sharper control transitions for readability.
7. Save the PNG and print the sampled control summary.

## Modes

- `composite`
  - default broad-shape visualization combining uplift, flatness, wetness, ridge, terrace, and corridor influence
- `flatness`
- `relief`
- `uplift`
- `wetness`
- `ridge`
- `terrace`
- `corridor`

## Scale Semantics

- default scale is `--blocks-per-pixel 8`
- with the current `32 x 32` chunk edge, that means one pixel covers one quarter-chunk footprint
- equivalently:
  - `8 x 8` world blocks per pixel
  - `4 x 4` pixels per chunk
- changing `--blocks-per-pixel` changes both sampling density and final image dimensions

## Transitional Notes

- the current binary now uses the public realization-field API exposed from `world::generation`
- sampled biome / archetype / terrain-form counts still come from region classification on purpose, because realization field diffuses parameters rather than semantic ids
- channel views are proxies over the current `RealizationSample` fields:
  - `wetness` visualizes `wet_flatten`
  - `corridor` visualizes corridor susceptibility / policy channels, not final corridor geometry or hydrology carve
