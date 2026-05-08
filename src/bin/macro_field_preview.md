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
  - `--chunk-radius <i32>`
  - `--region-size-blocks <i32>`
  - `--site-spacing-blocks <i32>`
  - `--land-bias <f32>`
  - `--stage macro_field`
  - `--channel <all|macro|mask|ridge|river|combined|lit|contour>`
  - `--contour-step <blocks>`
  - `--contour-major-every <n>`
  - `--contours`
  - `--output <path>`

## Defaults

- `--width 3840`
- `--height 2160`
- `--world-span-blocks 32768`
- default footprint equals `--chunk-radius 512` with the current `CHUNK_EDGE = 32`, but the default
  remains expressed as `--world-span-blocks 32768` for compatibility
- `--region-size-blocks DEFAULT_GRAPH_REGION_SIZE_BLOCKS`
- `--site-spacing-blocks DEFAULT_SITE_SPACING_BLOCKS`
- `--land-bias MacroMapConfig::new(...).land_bias`
- `--stage macro_field`
- `--channel lit`
- `--contour-step 32`
- `--contour-major-every 5`
- single-channel output: `target/macro-field-preview/s<seed>_x<center-x>_z<center-z>_<channel>.png`
- all-channel output directory: `target/macro-field-preview/s<seed>_x<center-x>_z<center-z>/`

## Channels

- `macro`: signed macro elevation sampled through noisy-boundary owner/blend logic.
- `mask`: ocean, lake/wetland, dry basin, explicit ocean coast, and land context following noisy
  boundaries. Dry basin is a land-owned closed basin context, not a shoreline.
- `ridge`: connected distance-envelope influence around ridge noisy boundary curves. It should read
  as a mountain belt shoulder around the ridge maxima guide, not isolated bright pixels and not
  global low-level texture.
- `river`: flow-scaled flat-bottom valley influence around selected hydrology river curves.
  Upstream segments are narrow and shallow but should not read as knife-cut V shapes; downstream
  trunks are wider with flatter beds and broader shoulders.
- `combined`: macro elevation minus visible river valley carve guide, coast flatten, and water
  flatten, rendered as a subtle terrain ramp rather than a diagnostic heat map. Ridge influence is
  diagnostic-only in the current launch slice and does not raise combined height until a broader
  mountain elevation model is reintroduced.
- `lit`: top-down white heightfield preview with broad directional hillshade from combined height
  gradients. This is not a 3D render and it is not per-tile lighting; it is shaded relief over the
  overall combined macro height field.
- `contour`: Marching Squares contour lines extracted from `combined_macro_height` after converting
  it to the same block-height scale used by the current heightfield launch slice. Minor contours use
  `--contour-step`, major contours use `--contour-step * --contour-major-every`, and sea level
  `y=0` is drawn in a separate muted blue. Other contour lines use a height color ramp: low/oceanward
  levels are blue and high levels move toward orange/red. Major contours keep the same height hue
  but render stronger.

`macro`, `combined`, and `lit` use a fixed absolute normalized preview scale rather than per-image
min/max stretching. The launch preview scale is:

```text
macro elevation: -1.00 .. 1.00
combined height: -0.50 .. 1.00
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
  `combined.png`, `lit.png`, and `contour.png`.
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
  - ridge candidate noisy curves through a tile-local source-pixel/chamfer influence raster pass,
  - coast noisy curves through a tile-local source-pixel/chamfer influence raster pass,
  - selected hydrology river noisy curves through a tile-local anti-aliased thick polyline bake
    that stores smooth valley coverage, nearest-segment distance, and blended display flow.
8. Extract optional contour diagnostics from the world-owned `MacroFieldTile` combined height:
   effective `combined_macro_height -0.5..1.0 -> -1024..2048 blocks`, with the central
   `-0.25..0.75` interest range mapping to `-512..1536 blocks`.
9. Render the world-owned `MacroFieldTile` in parallel over the image sample grid.
10. Render the requested channel or all channels with a compact legend, canonical noisy Voronoi edge
   overlay, a scale bar, a compass overlay, and a thin macro-field cache tile grid.
11. Encode PNG metadata in `new-world-preview-header`.

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
- contour step, major interval, min/max level, level count, segment count, height color ramp, and overlay flag
- lit raw gradient stats, smoothed-normal gradient stats, and broad hillshade brightness
  min/average/max/stddev
- channel meaning notes for macro, mask, ridge, river, combined, contour, and lit outputs

## Interpretation Notes

- `lit` is still pre-Perlin. Any fine detail visible there comes from macro elevation gradients,
  noisy-boundary blend, ridge/coast/river influence, or the lighting contrast itself.
- `lit` uses smoothing only for preview lighting. It does not blur or rewrite
  `combined_macro_height`; the goal is to suppress sample-scale macro transitions while keeping
  broad continent/ridge/basin/coast height differences visible as white-material hillshade.
- `DryBasin` is not water. In `combined` and `lit`, it should read as a shallow closed land floor,
  not as a lake/ocean surface and not as a mandatory deep carve.
- Tiny 1..3-cell local-minima lakes are water/lake mask features even when no selected river reaches
  them. They should not be recolored or shaped as dry basin bowls in `mask`, `combined`, or `lit`.
- `combined` uses the same absolute height scale as before, but its colors should read like a
  top-down pre-Perlin terrain surface: muted blue-gray low/ocean values, subdued green-gray low
  land, olive/gray midlands, and pale gray high values without white saturation. It should not show
  narrow ridge-created pinpoint peaks while ridge raise is disabled.
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
- `--chunk-radius` keeps the positional center in world-block coordinates. It only changes the
  preview footprint to `radius * 2 * CHUNK_EDGE` blocks on the X axis, with Z derived from image
  aspect. Smaller radii therefore zoom in at the same image size. The current default footprint
  `32768` blocks is equivalent to radius `512`.
- The compass overlay is drawn on every channel. Its orientation is fixed to the common macro-field
  frame: image top=N, right=E, bottom=S, left=W.
- The `contour` channel is a diagnostic layer, not a terrain source of truth. It should be used to
  check whether the pre-heightfield combined macro height is continuous and readable before the
  heightfield/water solve consumes it. The contour channel keeps noisy Voronoi edge overlay off so
  contour continuity is not hidden by graph registration lines.

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

Contour smoke:

```bash
cargo run --release --bin macro_field_preview -- 42 0 0 --width 1280 --height 720 --channel contour --output target/macro-field-preview/contour.png
```

Combined terrain ramp with contour overlay:

```bash
cargo run --release --bin macro_field_preview -- 42 0 0 --width 1280 --height 720 --channel combined --contours --output target/macro-field-preview/combined-contour.png
```
