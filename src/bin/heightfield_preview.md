# heightfield_preview

## Role

- Render an isometric diagnostic preview for graph-first heightfield columns.
- Reuse the same generation chain as `macro_field_preview` through stage 8, then convert the
  `MacroFieldTile` into a `HeightfieldTile`.
- Voxelize heightfield columns into diagnostic block-like boxes without writing final `ChunkData`.

## Inputs

- positional: `<seed> <center-x> <center-z>`
  - `center-x` and `center-z` are world-block coordinates.
- optional:
  - `--width <u32>`: output image width, default `1280`
  - `--height <u32>`: output image height, default `720`
  - `--world-span-blocks <i32>`: horizontal world footprint, default `8192`
  - `--chunk-radius <i32>`: square chunk radius around the chunk containing `center-x/center-z`.
    When set, this overrides the preview footprint derived from `--world-span-blocks`; width/height
    only control image resolution.
  - `--columns-x <u32>`: sampled heightfield columns across X, default `192`
  - `--columns-z <u32>`: sampled heightfield columns across Z, default derived from aspect unless
    `--chunk-radius` is set, in which case it defaults to `columns-x` for a square sample grid
  - `--region-size-blocks <i32>`
  - `--site-spacing-blocks <i32>`
  - `--land-bias <f32>`
  - `--quarter-turns <u8>`: isometric camera rotation in 90 degree steps, default `0`
  - `--vertical-scale <f32>`: preview-only multiplier for automatic vertical relief fit, default `1.0`
  - `--output <path>`

## Flow

1. Build the padded Voronoi graph patch.
2. Resolve macro map.
3. Solve hydrology.
4. Generate canonical noisy boundaries.
5. Rasterize `MacroFieldTile` at the requested column resolution.
6. Convert it to `HeightfieldTile` with pure contour-band terrace resolve.
7. Snap heightfield surface/water output to integer block heights.
8. Project columns with a CPU 2D isometric column renderer. Water columns use the water surface as
   their visible top for neighbor-delta side faces; the underwater bed remains stored separately.
9. Draw visible side faces, top faces, water tops, the primary 1024-block macro-field tile grid,
   secondary/faint 256-block chunk-group references, very faint 32-block chunk boundaries, scale
   bar, and metadata legend in painter order.

## Interpretation

- This binary is not final voxel fill.
- Meso features and Perlin micro relief are currently stubbed as zero in `heightfield`.
- The default view is a CPU-rendered isometric column view, not a 3D orthographic camera. Projection
  is explicit:

```text
screen_x = (x - z) * tile_w / 2
screen_y = (x + z) * tile_h / 2 - y * vertical_px_per_block
```

- `vertical_px_per_block` is computed per preview so the visible height range occupies roughly
  20-35% of the image height. This keeps top faces and relief visible at the same time.
- `--vertical-scale` multiplies the automatic relief fit. It does not modify `HeightfieldTile`
  values or persisted generation data.
- Columns are drawn as top diamonds plus only the visible east/south side faces where a neighbor is
  lower. The preview does not draw every column down to a global base plane, because that reads as a
  side-view wall chart instead of a macro terrain surface.
- Colors are diagnostic and intentionally close to the subtle terrain ramp:
  - muted blue water/ocean
  - subdued green-gray low land
  - pale gray high/ridge
  - muted gray/mauve dry basin
- Water boxes come from heightfield water hints, not final fluid simulation.
- Sea level is fixed at `y = 0`. Coast-adjacent land uses a shoreline contour ceiling before
  integer snapping so ordinary ocean/land contact does not render as an immediate vertical wall.
  In pure contour-step mode, the first land ring next to water starts at `y = 0`, then rises inward
  by contour steps.
- Heightfield columns are resolved to contour bands before water/shore constraints. The raw block
  height from `combined_macro_height` remains stored for diagnostics, but final land surface does
  not directly use the continuous scalar. Default contour step is `1` block and smoothing is
  disabled, so the preview shows every one-block terrace. This is not contour-line reconstruction;
  it is scalar-to-band quantization in the same block-height domain as the contour preview.
- Ocean/lake terrain `surface_height_blocks` is bed height. The preview compares adjacent columns by
  visible top height, `max(surface_height_blocks, water_level_blocks)`, so a water bed does not look
  like a shoreline cliff.
- Grid overlay has three diagnostic layers, but the primary readable scale is the same
  `1024`-block macro-field tile grid used by `macro_field_preview`. The runtime chunk edge is
  currently `32` blocks and remains as a very faint reference. The `256`-block grid is a secondary
  chunk-group reference, equal to 8 chunks, and must not read stronger than the macro tile grid.
  These are distinct overlays: macro tile lines show generation cache scale, secondary major lines
  show chunk groups, and minor chunk lines show future `ChunkData` output windows.
- With `--chunk-radius r`, the preview range is inclusive in chunk coordinates:
  `center_chunk-r .. center_chunk+r` on both X and Z. A radius of `0` shows exactly the chunk that
  contains the world-block center. The world footprint is the covered chunk square times
  `CHUNK_EDGE`.
- The legend/header records `cx`, `cz`, world footprint, column count/spacing, chunk x/z range,
  chunk radius, height range, contour step/smoothing-disabled value, sea level, primary `macro tile 1024 blk`,
  secondary `major 256 blk`, faint `chunk 32 blk`, and a block scale bar.
- The legend scales from the output image dimensions. Its metadata panel targets about one fifth of
  the image height, and text, spacing, swatches, and scale bar grow proportionally with resolution.

## Example

```bash
cargo run --release --bin heightfield_preview -- 42 0 0 --width 1280 --height 720 --output target/heightfield-preview/heightfield-smoke.png
```

```bash
cargo run --release --bin heightfield_preview -- 42 0 0 --chunk-radius 32 --width 1280 --height 720 --output target/heightfield-preview/heightfield-r32.png
```
