# biome_map_preview

## Role

- Render a deterministic top-down PNG preview that colors each Voronoi cell by resolved biome.
- Follow the same world-block center, footprint, short filename, metadata, and compass conventions
  as `macro_map_preview`.
- Stay isolated to preview/bin ownership and read the core classifier output through the macro map.

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
  - `--stage biome_map`
  - `--output <path>`

## Defaults

- `--width 3840`
- `--height 2160`
- `--world-span-blocks 32768`
- `--region-size-blocks DEFAULT_GRAPH_REGION_SIZE_BLOCKS`
- `--site-spacing-blocks DEFAULT_SITE_SPACING_BLOCKS`
- `--land-bias MacroMapConfig::new(...).land_bias`
- `--stage biome_map`
- output: `target/biome-map-preview/s<seed>_x<center-x>_z<center-z>.png`

## Outputs

- One RGB PNG:
  - each pixel uses nearest-site diagnostic fill, matching the current macro map preview style;
  - every current `GraphBiomeKind` variant uses its own distinct color code, including split ocean,
    forest, cold, and alpine variants;
  - graph Voronoi edges are drawn as a faint actual corner-to-corner overlay;
  - a larger one-row-per-biome legend and common topdown compass are drawn over the image.
- A PNG iTXt chunk named `new-world-preview-header` records seed, generator version, stage, center,
  dimensions, graph area, site spacing, land bias, site count, and per-biome site counts.

## Current Flow

1. Build a padded Voronoi graph patch through `generate_voronoi_graph_patch(...)`.
2. Build the macro map through `generate_macro_map(&patch, MacroMapConfig::new(...))`.
3. Read resolved biome cells from the macro map:

```rust
GraphMacroMap::biome(site_id)
GraphMacroMap::biomes
GraphBiomeCell
GraphBiomeKind
```

The preview does not classify core biome policy itself. `macro_map` owns the graph biome context and
calls `classify_graph_biome(...)`; this binary only colors the returned cells.

## Output Path Rules

- With no `--output`, the binary writes `target/biome-map-preview/s<seed>_x<center-x>_z<center-z>.png`.
- With `--output <path>.png`, the binary writes that exact PNG path.
- With `--output <directory>`, the binary writes `s<seed>_x<center-x>_z<center-z>.png` below that directory.

## Example

```bash
cargo run --bin biome_map_preview -- 42 0 0
```

Lower-resolution smoke check:

```bash
cargo run --bin biome_map_preview -- 42 0 0 --width 640 --height 360 --output target/biome-map-preview/smoke.png
```
