# biome_cell_inspector

## Role

`biome_cell_inspector` writes a self-contained responsive HTML inspector for graph-first biome
cells.

The tool builds the same graph-first inputs used by `biome_map_preview`, colors Voronoi cells by
the resolved `GraphBiomeKind`, and overlays selected hydrology/lake diagnostics from the
macro-map preview path. Clicking a cell highlights it and updates the side panel with that cell's
final context values.

## Ownership

- Source of truth for biome classification remains `world::generation::biome`.
- Source of truth for macro surface, lake edge class, and selected hydrology remains
  `world::generation::{macro_map, hydrology, boundary}`.
- This binary only serializes those public generation outputs into HTML/SVG for inspection.

## Command

```bash
cargo run --bin biome_cell_inspector -- <seed> <center-x> <center-z> [options]
```

Options:

- `--width <u32>`: SVG viewBox width, default `1400`
- `--height <u32>`: SVG viewBox height, default `900`
- `--world-span-blocks <i32>`: covered world-space width, default `32768`
- `--region-size-blocks <i32>`: graph region size override
- `--site-spacing-blocks <i32>`: graph site spacing override
- `--land-bias <f32>`: forwarded to `MacroMapConfig`
- `--stage biome_cell_inspector`
- `--output <path>`: writes the exact `.html` file when the path has an extension; otherwise
  appends the default file name under that directory

Example:

```bash
cargo run --bin biome_cell_inspector -- 42 0 0 --output target/biome-cell-inspector/s42.html
```

## Display Contract

- Cell fill color matches the detailed biome palette used by `biome_map_preview`.
- Pale cyan lake strokes come from macro lake edge classes.
- Cyan river strokes come from selected `GraphHydrologyGraph` river segments and use selected
  display flow, not raw flow. The inspector draws them thinner than the macro-map composite so
  they stay readable over dense cell polygons.
- Lake inlet/outlet/sink/coast outlet markers are exposed as overlay diagnostics.
- The side panel exposes at least:
  - biome
  - macro surface kind
  - water role
  - temperature
  - hydration
  - elevation
  - continentality
  - coastness
  - mountainness
  - ruggedness
  - macro site kind
  - macro lake edge counts
  - selected river segment count and max selected flow

## Limitations

- The generated artifact is HTML/SVG, not a native GUI.
- Voronoi cells are drawn from sorted raw corner polygons for inspection. The visible biome
  boundary smoothing/noisy-domain transition belongs to later generation stages.
- The binary does not create preview PNGs and does not change lake or biome classification
  semantics.
