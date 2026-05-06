# heightfield_preview

## Role

- Render a quarter-view diagnostic preview for graph-first heightfield columns.
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
  - `--quarter-turns <u8>`: quarter-view rotation, default `0`
  - `--vertical-scale <f32>`: preview-only vertical exaggeration, default `6.0`
  - `--output <path>`

## Flow

1. Build the padded Voronoi graph patch.
2. Resolve macro map.
3. Solve hydrology.
4. Generate canonical noisy boundaries.
5. Rasterize `MacroFieldTile` at the requested column resolution.
6. Convert it to `HeightfieldTile`.
7. Build diagnostic terrain and water box meshes from columns.
8. Render with the offscreen quarter-view renderer.

## Interpretation

- This binary is not final voxel fill.
- Meso features and Perlin micro relief are currently stubbed as zero in `heightfield`.
- Colors are diagnostic and intentionally close to the subtle terrain ramp:
  - muted blue water/ocean
  - subdued green-gray low land
  - pale gray high/ridge
  - muted gray/mauve dry basin
- Water boxes come from heightfield water hints, not final fluid simulation.

## Example

```bash
cargo run --release --bin heightfield_preview -- 42 0 0 --width 1280 --height 720 --output target/heightfield-preview/heightfield-smoke.png
```
