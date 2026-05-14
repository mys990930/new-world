# pixelize_preview

## Role

`pixelize_preview` renders the graph-first `pixelize` stage as a top-down PNG over a chunk-aligned
square footprint.

The preview is a binary adapter. It builds graph, macro map, hydrology, noisy boundary, and macro
field inputs through public `world::generation` APIs, then delegates chunk-aligned column resolve to
the `pixelize` API. It does not reinterpret graph topology, hydrology, noisy boundary ownership, or
height policy in the binary.

## CLI

```bash
cargo run --bin pixelize_preview -- <seed> <cx> <cz> <r> [options]
```

Positional arguments:

- `<seed>`: world seed.
- `<cx>`: center chunk x coordinate.
- `<cz>`: center chunk z coordinate.
- `<r>`: inclusive square chunk radius. `0` previews one chunk; `1` previews a `3 x 3` chunk
  square.

Options:

- `--width <u32>`: output image width. Defaults to one output pixel per pixelized world-block
  column.
- `--height <u32>`: output image height. Defaults to one output pixel per pixelized world-block
  column.
- `--region-size-blocks <i32>`: graph region size. Defaults to
  `DEFAULT_GRAPH_REGION_SIZE_BLOCKS`.
- `--site-spacing-blocks <i32>`: graph site spacing. Defaults to `DEFAULT_SITE_SPACING_BLOCKS`.
- `--land-bias <f32>`: forwarded to `MacroMapConfig.land_bias`.
- `--stage pixelize`: accepted stage name; any other value is rejected.
- `--output <path>`: explicit PNG path, or a directory when the path has no extension.

Default output:

```text
target/pixelize-preview/s<seed>_cx<cx>_cz<cz>_r<r>.png
```

## Output

- The default PNG has one rendered pixel per pixelized world-block column.
- If `--width` or `--height` is supplied, the already pixelized chunk area is nearest-neighbor
  scaled into a centered square map viewport; the data itself remains one column per world block.
  Rectangular outputs preserve the square chunk footprint and fill the unused side or top/bottom
  bands with a neutral letterbox color.
- Color is a diagnostic height ramp with water, river, coast, dry basin, and ridge hints layered on
  top.
- Chunk grid lines are drawn every `CHUNK_EDGE` blocks inside the square map viewport.
- The render overlays stage 9 canonical noisy boundary curves from `BoundaryCache`. Each noisy
  polyline segment is clipped to the requested chunk footprint, snapped to the pixelized
  column/border lattice, expanded into a 4-connected orthogonal stair-step path, and then projected
  into the square map viewport. The overlay follows pixelize units exactly instead of drawing
  diagonal strokes across resolved columns.
- A compact legend and north-up/east-right compass are included.
- PNG metadata is written to the `new-world-preview-header` iTXt chunk.

## Pipeline

The binary builds inputs in this order:

1. `generate_voronoi_graph_patch`
2. `generate_macro_map`
3. `solve_hydrology`
4. `apply_headwater_source_hydration_to_biomes`
5. `generate_noisy_boundaries`
6. `generate_macro_field_tile`
7. `generate_pixelized_chunk_area`

The macro field tile is sampled at one block spacing over the requested inclusive chunk area.

## API Dependency

This binary expects the core pixelize module to expose names close to:

```rust
PixelizeConfig::default()
generate_pixelized_chunk_area(&MacroFieldTile, PixelizeConfig) -> PixelizedChunkArea
PixelizedChunkArea { width, height, columns, .. }
PixelizedColumn {
    surface_y,
    water_y,
    terrain_kind,
    source_combined_macro_height,
    source_ocean_mask,
    source_lake_mask,
    source_coast_mask,
    source_dry_basin_mask,
    source_river_valley_strength,
    source_ridge_influence,
    ..
}
```

The preview binary renders this API output directly and does not own any additional pixelize policy.
