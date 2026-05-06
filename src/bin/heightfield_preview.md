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
  - `--columns-x <u32>`: sampled heightfield columns across X, default `192`
  - `--columns-z <u32>`: sampled heightfield columns across Z, default derived from aspect
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
6. Convert it to `HeightfieldTile`.
7. Snap heightfield surface/water output to integer block heights.
8. Project columns with a CPU 2D isometric column renderer.
9. Draw visible side faces, top faces, water tops, chunk boundaries, macro-field tile boundaries,
   scale bar, and metadata legend in painter order.

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
- Sea level is fixed at `y = 0`. Coast-adjacent land is shoreline-ramped before integer snapping so
  ordinary ocean/land contact does not render as an immediate vertical wall.
- Chunk overlay uses the runtime chunk edge size, currently `32` blocks. Macro-field tile overlay
  uses the graph/cache tile scale, currently `1024` blocks. They are distinct diagnostic overlays:
  chunk lines show future `ChunkData` output windows, macro tile lines show generation cache scale.
- The legend/header records `cx`, `cz`, world footprint, column count/spacing, chunk x/z range,
  chunk radius, height range, sea level, and a block scale bar.

## Example

```bash
cargo run --release --bin heightfield_preview -- 42 0 0 --width 1280 --height 720 --output target/heightfield-preview/heightfield-smoke.png
```
