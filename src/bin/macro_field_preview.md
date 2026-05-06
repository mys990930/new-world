# macro_field_preview

## Role

- Render deterministic top-down PNG previews for the stage 8 macro field rasterization step.
- Treat graph, macro map, hydrology, and noisy boundary output as the source of truth, then bake a
  preview tile that heightfield synthesis can sample cheaply.
- Use the world-owned `macro_field` tile API rather than redefining preview-only terrain sampling.
- Keep default filenames short while preserving detailed settings and stage statistics in PNG
  metadata.

## Inputs

- positional: `<seed> <center-x> <center-z>`
  - `center-x` and `center-z` are world-block coordinates.
- optional:
  - `--width <u32>`
  - `--height <u32>`
  - `--world-span-blocks <i32>`
  - `--region-size-blocks <i32>`
  - `--site-spacing-blocks <i32>`
  - `--land-bias <f32>`
  - `--stage macro_field`
  - `--channel <all|macro|mask|ridge|river|combined|lit>`
  - `--output <path>`

## Defaults

- `--width 3840`
- `--height 2160`
- `--world-span-blocks 32768`
- `--region-size-blocks DEFAULT_GRAPH_REGION_SIZE_BLOCKS`
- `--site-spacing-blocks DEFAULT_SITE_SPACING_BLOCKS`
- `--land-bias MacroMapConfig::new(...).land_bias`
- `--stage macro_field`
- `--channel lit`
- single-channel output: `target/macro-field-preview/s<seed>_x<center-x>_z<center-z>_<channel>.png`
- all-channel output directory: `target/macro-field-preview/s<seed>_x<center-x>_z<center-z>/`

## Channels

- `macro`: signed macro elevation sampled through noisy-boundary owner/blend logic.
- `mask`: ocean, lake/wetland, dry basin, explicit ocean coast, and land context following noisy
  boundaries. Dry basin is a land-owned closed basin context, not a shoreline.
- `ridge`: distance-envelope influence around ridge noisy boundary curves.
- `river`: distance-envelope valley influence around selected hydrology river curves.
- `combined`: macro elevation plus ridge raise, minus visible river valley carve guide, coast
  flatten, and water flatten.
- `lit`: top-down white heightfield preview with broad directional hillshade from combined height
  gradients. This is not a 3D render and it is not per-tile lighting; it is shaded relief over the
  overall combined macro height field.

`macro`, `combined`, and `lit` use a fixed absolute normalized preview scale rather than per-image
min/max stretching. The launch preview scale is:

```text
macro elevation: -1.00 .. 1.00
combined height: -0.75 .. 1.25
```

The renderer still records per-image min/max/robust percentiles as diagnostics, but those values do
not drive the color ramp. This keeps adjacent macro-field tiles in the same terrain context instead
of giving every tile its own artificial low and high.

## Output Path Rules

- With no `--output`, a single channel writes the short default PNG path.
- With `--output <path>.png`, a single channel writes that exact PNG path.
- With `--output <directory>`, a single channel writes `<channel>.png` below that directory.
- In `--channel all`, `--output <directory>` writes one short PNG per channel in that directory.
- In `--channel all`, `--output <path>.png` is interpreted as a prefix directory using the file stem,
  so `--output target/macro-field-preview/field-smoke.png` writes
  `target/macro-field-preview/field-smoke/macro.png`, `mask.png`, `ridge.png`, `river.png`,
  `combined.png`, and `lit.png`.
- Width, height, span, spacing, stage, and generator version stay in PNG metadata rather than
  default filenames.

## Current Flow

1. Parse the seed and world-block center.
2. Resolve the preview window from image dimensions and `--world-span-blocks`.
3. Build the padded Voronoi graph through `generate_voronoi_graph_patch(...)`.
4. Resolve macro ownership/elevation through `generate_macro_map(...)`.
5. Solve selected hydrology through `solve_hydrology(...)`.
6. Generate canonical noisy boundaries through `generate_noisy_boundaries(...)`.
7. Build a `MacroFieldTile` through `generate_macro_field_tile(...)`. The tile samples:
  - noisy-boundary owner/blend for macro elevation and masks,
   - ridge candidate noisy curves through a tile-local influence raster pass,
   - coast noisy curves through a tile-local influence raster pass,
   - selected hydrology river noisy curves through a tile-local influence raster pass.
8. Render the world-owned `MacroFieldTile` in parallel over the image sample grid.
9. Render the requested channel or all channels with a compact legend, canonical noisy Voronoi edge
   overlay, a scale bar, and a thin macro-field cache tile grid.
10. Encode PNG metadata in `new-world-preview-header`.

## Integration Note

The preview must not redefine macro terrain semantics. It reads:

```rust
generate_voronoi_graph_patch(...)
generate_macro_map(...)
solve_hydrology(...)
generate_noisy_boundaries(...)
generate_macro_field_tile(...)
```

and renders that world-owned tile into diagnostic 2D fields.

## Metadata

Each PNG contains:

- seed, generator version, stage, channel, center, dimensions, world span, graph area, spacing, and
  land bias
- graph site count, macro edge count, boundary curve count, selected river feature sample count
- ridge, river, and coast feature sample counts
- ridge, river, and coast source curve/pixel counts from the influence raster pass
- ridge active sample count/fraction so low-level ridge tails cannot masquerade as micro detail
- dry basin sample count plus combined-height min/max/average so dry basins can be checked as
  shallow land floors rather than water-flattened holes
- macro field tile generation timing plus render/encode timing
- noisy boundary average/max displacement
- min/max/average plus robust preview contrast range for macro elevation, ridge influence, river
  valley, and combined macro height
- fixed absolute preview scale, white saturation fraction, and tile boundary grid spacing/count
- noisy Voronoi edge overlay curve/segment count and scale bar length
- lit raw gradient stats, smoothed-normal gradient stats, and broad hillshade brightness
  min/average/max/stddev
- channel meaning notes for macro, mask, ridge, river, combined, and lit outputs

## Interpretation Notes

- `lit` is still pre-Perlin. Any fine detail visible there comes from macro elevation gradients,
  noisy-boundary blend, ridge/coast/river influence, or the lighting contrast itself.
- `lit` uses smoothing only for preview lighting. It does not blur or rewrite
  `combined_macro_height`; the goal is to suppress sample-scale macro transitions while keeping
  broad continent/ridge/basin/coast height differences visible as white-material hillshade.
- `DryBasin` is not water. In `combined` and `lit`, it should read as a shallow closed land floor,
  not as a lake/ocean surface and not as a mandatory deep carve.
- In `mask`, the yellow/sandy key means explicit ocean coast only. Dry basin uses its own muted
  mauve/gray key and must not be inferred from the coast color or coast gradient band.
- The primary boundary overlay is the canonical noisy Voronoi graph edge layer, but it must remain a
  faint reference overlay so the field value stays visually dominant. The cache grid is diagnostic
  only: it should reveal cache boundaries without implying that terrain height is normalized
  independently inside each tile.
- `lit` uses an even fainter Voronoi edge overlay than the other channels. Its first job is to show
  broad white-material hillshade, so graph edges there are only a barely visible registration aid.
- The scale bar is drawn on every channel so the world footprint can be read without checking
  metadata.

## Example

```bash
cargo run --bin macro_field_preview -- 42 0 0
```

All-channel smoke output:

```bash
cargo run --bin macro_field_preview -- 42 0 0 --width 640 --height 360 --channel all --output target/macro-field-preview/field-smoke.png
```

Release timing smoke:

```bash
cargo run --release --bin macro_field_preview -- 42 0 0 --width 1280 --height 720 --channel lit --output target/macro-field-preview/field-splat-720p.png
```
