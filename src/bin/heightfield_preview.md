# heightfield_preview

## Role

- Render an isometric diagnostic preview for graph-first heightfield columns.
- Reuse the same generation chain as `macro_field_preview` through stage 8, then convert the
  `MacroFieldTile` into a `HeightfieldTile`.
- Voxelize heightfield columns into diagnostic block-like boxes without writing final `ChunkData`.

## Inputs

- positional: `<seed> <center-x> <center-z>`
  - `center-x` and `center-z` are chunk coordinates by default.
  - Use `--world-center` or `--world-coordinates` only when you intentionally want the old
    world-block center interpretation.
- optional:
  - `--width <u32>`: output image width, default `1280`
  - `--height <u32>`: output image height, default `720`
  - `--world-span-blocks <i32>`: horizontal world footprint, default `8192`
  - `--chunk-radius <i32>`: square chunk radius around the positional center chunk. When set, this
    overrides the preview footprint derived from `--world-span-blocks`; width/height only control
    image resolution.
  - `--columns-x <u32>`: sampled heightfield columns across X. Free-window mode defaults to `768`;
    `--chunk-radius` mode defaults to `(2r+1) * 64`.
  - `--columns-z <u32>`: sampled heightfield columns across Z, default derived from aspect unless
    `--chunk-radius` is set, in which case it defaults to `columns-x` for a square sample grid.
    Explicit `--columns-x`/`--columns-z` values always own the final resolution.
  - `--region-size-blocks <i32>`
  - `--site-spacing-blocks <i32>`
  - `--land-bias <f32>`
  - `--quarter-turns <u8>`: isometric camera rotation in 90 degree steps, default `0`
  - `--block-lines` / `--no-block-lines`: enable or disable very thin diagnostic outlines around
    each rendered column top/visible side. The default is on. Visible side faces also receive
    one-pixel integer `y` step guides so individual block layers remain readable in
    `--chunk-radius 1` previews.
  - `--output <path>`

## Flow

1. Build the padded Voronoi graph patch.
2. Resolve macro map.
3. Solve hydrology.
4. Generate canonical noisy boundaries.
5. Rasterize `MacroFieldTile` at the requested column resolution.
6. Convert it to `HeightfieldTile` with contour-band terrace resolve. The preview does not use a
   hidden X/Z scale layer; the `MacroFieldTile` column count and `sample_spacing_blocks` directly
   define the horizontal density while Y block height is resolved in the shared block-domain before
   rendering.
7. Snap heightfield surface/water output to integer block heights. Ocean/lake visible surface is
   fixed at `y = 0`; this vertical slice does not render ocean bathymetry.
8. Project columns with a CPU 2D isometric column renderer. Water columns use the water surface as
   their visible top for neighbor-delta side faces.
9. Draw visible side faces, top faces, water tops, the primary 1024-block macro-field tile grid,
   secondary/faint 256-block chunk-group references, very faint 32-block chunk boundaries, scale
   bar, metadata legend, and compass overlay in painter order.

## Interpretation

- This binary is not final voxel fill.
- Meso features and Perlin micro relief are currently stubbed as zero in `heightfield`.
- The default view is a CPU-rendered isometric column view, not a 3D orthographic camera. Projection
  is explicit:

```text
screen_x = (x - z) * tile_w / 2
screen_y = (x + z) * tile_h / 2 - y * vertical_px_per_block
```

- `vertical_px_per_block`은 preview 렌더링 전용 값이지만, XZ density에 맞춰 같은 Y 값을 다시 낮추는
  normalization 계수가 아니다. 이 binary는 block primitive가 화면에서 정육면체에 가깝게 읽히도록
  cubic scale로 렌더한다.
- Macro relief는 preview 렌더링이 아니라 `macro_field` contour와 `heightfield` band resolve가
  공유하는 block-height domain에서 이미 산출된다. 현재 기본 scale은
  `combined_macro_height -0.75..0.0..1.25 -> -64..0..224 blocks`이며, preview에서 같은 Y 값을 다시
  낮춰 그리면 중복 압축이다.
- Horizontal density is a direct column-count contract. In free-window mode the default X column
  count is `768`; Z is derived from the image aspect unless `--columns-z` is provided. In
  `--chunk-radius` mode, the default grid uses `64` columns per chunk on each axis, so a radius
  `r` covers `(2r+1) * 64` columns per axis unless `--columns-x`/`--columns-z` explicitly override
  the final count. Sea level, contour step, surface `y`, and river water `y` are resolved in the
  block domain before rendering. X/Z 화면 픽셀 스케일도 별도 조절값을 갖지 않고 column count, footprint,
  image size에서 자동으로 파생된다.
- Columns are drawn as top diamonds plus only the visible side faces where a neighbor is lower. The
  visible sides are derived from the current `--quarter-turns` projection. For example, quarter `0`
  sees the +X/+Z faces, quarter `1` sees -X/+Z, quarter `2` sees -X/-Z, and quarter `3` sees +X/-Z.
  The preview does not draw every column down to a global base plane, because that reads as a
  side-view wall chart instead of a macro terrain surface.
- Columns are depth-sorted by projected horizontal depth after applying `--quarter-turns`. A fixed
  `x+z` painter order is a regression because it only works for one quarter view.
- Very thin block lines are drawn on top/visible side polygons by default to make the block scale
  readable at dense column counts. Top faces get a subtle face-edge outline, visible side faces
  get a subtle face-edge outline plus one-pixel horizontal guides at integer `y` block steps. They
  are diagnostic overlay lines, not final mesh edges.
- Colors are diagnostic and intentionally close to the subtle terrain ramp:
  - muted blue water/ocean
  - subdued green-gray low land
  - pale gray high/ridge
  - muted gray/mauve dry basin
- Water boxes come from heightfield water hints, not final fluid simulation.
- Sea level is fixed at `y = 0`. Ocean/lake columns render their visible surface at `y = 0`; the
  current vertical slice hides bathymetry so the sea side does not read as carved terrain.
- Coast-adjacent land uses a shoreline contour ceiling before integer snapping so ordinary
  ocean/lake contact does not render as an immediate vertical wall. In pure contour-step mode, the
  first land ring next to standing water starts at `y = 0`, then rises inward by one-block contour
  steps.
- Heightfield columns are resolved to contour bands before water/shore constraints. The raw block
  height from `combined_macro_height` remains stored for diagnostics, but final land surface does
  not directly use the continuous scalar. Default contour step is `1` block, and both general land
  and river corridors use a `1` block minimum raw gap. The visible result still uses integer
  terraces, but a raw one-block interval alone does not open a new visible height; with the default
  `step + min_gap` stride, raw height advances by 2 blocks per visible 1-block terrace. This is not
  contour-line reconstruction; it is scalar-to-band quantization in the same block-height domain as
  the contour preview.
- River columns receive an integer preliminary water height. Before preview, neighboring river or
  standing-water surfaces clamp river water so adjacent river-water steps descend by at most one
  block. This is a diagnostic vertical slice, not the final fluid/voxel channel solve.
- Ocean/lake terrain `surface_height_blocks` is the visible waterline surface in this slice, not
  bed height. The preview still compares adjacent columns by visible top height,
  `max(surface_height_blocks, water_level_blocks)`.
- Grid overlay has three diagnostic layers, but the primary readable scale is the same
  `1024`-block macro-field tile grid used by `macro_field_preview`. The runtime chunk edge is
  currently `32` blocks and remains as a very faint reference. The `256`-block grid is a secondary
  chunk-group reference, equal to 8 chunks, and must not read stronger than the macro tile grid.
  These are distinct overlays: macro tile lines show generation cache scale, secondary major lines
  show chunk groups, and minor chunk lines show future `ChunkData` output windows.
- With `--chunk-radius r`, the preview range is inclusive in chunk coordinates:
  `center_chunk-r .. center_chunk+r` on both X and Z. A radius of `0` shows exactly the positional
  center chunk. The world footprint is the covered chunk square times `CHUNK_EDGE`.
- When `--output` is omitted, the auto filename includes the projected quarter and chunk radius
  suffix, for example `s42_cx0_cz0_q0_r4.png`. Explicit `--output` paths are respected exactly.
- The legend/header records input center, input unit, center chunk, center world block, world
  footprint, column count/spacing, chunk x/z range, columns-per-chunk or explicit column override,
  chunk radius, height range, contour step/smoothing-disabled value, sea
  level, block outline state, primary `macro tile 1024 blk`, secondary `major 256 blk`, faint
  `chunk 32 blk`, and a block scale bar.
- The legend scales from the output image dimensions. Its metadata panel targets about one fifth of
  the image height, and text, spacing, swatches, and scale bar grow proportionally with resolution.
- The compass overlay follows the isometric projection after `--quarter-turns`. It does not stay
  topdown north-up; each N/E/S/W label is placed in the screen-space direction that the corresponding
  world cardinal axis projects to for the current quarter view.

## Example

```bash
cargo run --release --bin heightfield_preview -- 42 0 0 --width 1280 --height 720 --output target/heightfield-preview/heightfield-smoke.png
```

```bash
cargo run --release --bin heightfield_preview -- 42 0 0 --chunk-radius 8 --width 1280 --height 720 --output target/heightfield-preview/heightfield-r8.png
```

Quarter-view smoke set:

```bash
cargo run --release --bin heightfield_preview -- 42 0 0 --chunk-radius 8 --quarter-turns 0 --output target/heightfield-preview/s42_c0_0_r8_q0.png
cargo run --release --bin heightfield_preview -- 42 0 0 --chunk-radius 8 --quarter-turns 1 --output target/heightfield-preview/s42_c0_0_r8_q1.png
cargo run --release --bin heightfield_preview -- 42 0 0 --chunk-radius 8 --quarter-turns 2 --output target/heightfield-preview/s42_c0_0_r8_q2.png
cargo run --release --bin heightfield_preview -- 42 0 0 --chunk-radius 8 --quarter-turns 3 --output target/heightfield-preview/s42_c0_0_r8_q3.png
```
