# surface_plan_preview

## Role

- Render an isometric diagnostic preview for graph-first `surface_plan` columns.
- Reuse the same generation chain as `heightfield_preview` through `MacroFieldTile` and
  `HeightfieldTile`, then call `generate_surface_plan_area(...)`.
- Make material policy visible before final `ChunkData` voxel fill.

## Inputs

- positional: `<seed> <center-x> <center-z>`
  - `center-x` and `center-z` are chunk coordinates.
- optional:
  - `--chunk-radius <i32>`: square chunk radius around the positional center chunk, default `0`
  - `--width <u32>`: output image width, default `1280`
  - `--height <u32>`: output image height, default `720`
  - `--region-size-blocks <i32>`
  - `--site-spacing-blocks <i32>`
  - `--land-bias <f32>`
  - `--quarter-turns <u8>`: isometric camera rotation in 90 degree steps, default `0`
  - `--perlin`: backward-compatible no-op alias for the same default-on heightfield-owned Perlin
    micro relief used by `heightfield_preview`.
  - `--output <path>`

## Flow

1. Convert the center chunk and radius into an inclusive chunk square footprint.
2. Build the graph-first preview world in the documented order: graph, macro map, hydrology,
   headwater biome hydration, noisy boundary, and river plan.
3. Rasterize a `MacroFieldTile` at one column per world block for the selected chunk footprint.
4. Convert the macro tile into a `HeightfieldTile` with the same preview heightfield config as
   `heightfield_preview`; bounded heightfield-owned micro relief is enabled before surface material
   policy is applied. `--perlin` remains accepted for older scripts but does not change the
   default-on configuration.
5. Call `generate_surface_plan_area(&heightfield, Some(&macro_tile), SurfacePlanConfig::new(seed, generator_version))`.
6. Render terrain as isometric columns using `SurfaceColumnPlan.top_block` for top faces and
   `SurfaceColumnPlan.base_block` for visible side faces. The diagnostic palette keeps side-face
   base materials visually distinct from grass/sand top materials where possible, e.g. `sandstone`
   reads as rock/stone and `dirt`/`soil`/`humus` read as earth rather than vegetation.
7. Render `SurfaceColumnPlan.water_y` as a translucent blue overlay after the terrain pass.
8. Render a 1x1x4 player diagnostic cube at the center of the preview footprint, seated on the
   visible terrain/water surface.
9. Draw cyan noisy Voronoi boundary curves from `BoundaryCache`, clipped to the preview footprint
   and draped over the visible heightfield/water surface.
10. Write a PNG with metadata and print stdout diagnostics.

## Interpretation

- This binary is not final voxel fill.
- The preview color palette is material-key diagnostic color, not renderer material upload.
- Geometry is inherited from `HeightfieldTile`: `SurfaceColumnPlan.surface_y` and `water_y` come
  from the generated heightfield columns. Surface plan changes block/material interpretation on top
  of that geometry and must not recompute height from macro samples.
- Perlin matches `heightfield_preview` by building `HeightfieldConfig` with
  `HeightfieldPerlinConfig::preview_enabled(seed, generator_version)` by default. `--perlin` remains
  accepted as a backward-compatible no-op alias.
- The projection mirrors `heightfield_preview`:

```text
screen_x = (x - z) * tile_w / 2
screen_y = (x + z) * tile_h / 2 - y * vertical_px_per_block
```

- Column draw order is derived from the projected quarter-view depth, so changing
  `--quarter-turns` also changes the visible sides and painter order.
- Water is a translucent overlay over the resolved terrain bed. It does not replace the bed top.
- Vertical terrain walls are side-face diagnostics. Their colors come from base materials, not
  owner/top material, so they should not be interpreted as horizontal biome/material protrusions.
- Cyan boundary lines are diagnostic overlays only. They show the canonical noisy Voronoi boundary
  cache on top of the resolved visible surface and do not imply final material seams.
- The green/cyan player cube is a scale and camera-footprint diagnostic, not a gameplay entity.
- Default output path is `target/surface-plan-preview/s<seed>_cx<cx>_cz<cz>_q<q>_r<r>.png`.

## Example

```bash
cargo run --bin surface_plan_preview -- 42 0 0 --chunk-radius 0 --width 640 --height 360 --output target/surface-plan-preview/smoke.png
```

```bash
cargo run --bin surface_plan_preview -- 42 0 0 --chunk-radius 0 --perlin --width 640 --height 360 --output target/surface-plan-preview/perlin-smoke.png
```

## Current coordination note

This binary depends on the core `world::generation::surface_plan` API described by
`src/world/generation/surface_plan/surface_plan.md`. If that module has not landed yet,
`cargo check --bin surface_plan_preview` will report the missing API and should be retried after the
surface plan owner exports it.
