# bin

## Role

- Index the standalone binaries under `src/bin`.
- Summarize what each binary does, which parameters it accepts, and which command shape is recommended right now.
- Record current runtime status so callers can tell which binaries are fully usable, prototype-only, created-world-only, or still blocked by generation `todo!` paths.

## Current Index

| Binary | Purpose | Current status |
| --- | --- | --- |
| `atlas_proto` | Atlas debug image dump for raw / resolved atlas layers | Works |
| `atlas_realization_chunk_topdown_preview` | One-image atlas → realization → final chunk top-down comparison | Works |
| `chunk_preview` | Quarter-view isometric chunk preview | Recommended in `--stage prototype` or `--stage hydrology`; direct-seed `full` and `--lod-blocks > 1` are currently blocked by generation TODOs |
| `chunk_topdown_preview` | Exact top-down realized block-column preview | Works with an existing created-world dump; direct-seed mode currently depends on `generate_chunk(...)` TODO |
| `graph_voronoi_preview` | 4K top-down graph-first Voronoi macro graph and site-field preview maps | Works |
| `macro_map_preview` | Top-down graph-first macro map composite with ocean/land/elevation and candidate edge overlays | Works |
| `realization_field_preview` | Top-down realization/control-field preview | Works |
| `region_topdown_preview` | Top-down atlas region-classification preview | Works |
| `terrain_corridor` | Atlas-scale ocean-to-mountain corridor search and chart | Works |
| `terrain_find` | Search for chunk candidates by launch archetype and meso preferences | Works |
| `terrain_probe` | Per-chunk / per-column generation probe dump | Currently blocked by `probe_chunk(...)` and `probe_column(...)` TODO |
| `tree_preview` | Quarter-view preview of five generated variants for one climate tree blueprint | Works |
| `world_coords` | Inspect an existing created-world manifest and print preview coordinates | Works if `manifest.toml` already exists |
| `world_create` | Generate and persist a created-world dump | Currently blocked by `generate_chunk(...)` TODO |

## atlas_proto

- Purpose: write atlas debug PNGs for one atlas window.
- Parameters:
  - positional: `<seed> <width> [height]`
  - optional: `--origin-x <i32>`, `--origin-z <i32>`, `--pixels <u32>`, `--output <path>`
- Defaults:
  - `height = width`
  - `origin_x = -(width / 2)`
  - `origin_z = -(height / 2)`
  - `--pixels 4`
  - `--output target/atlas-debug/seed_<seed>_<width>x<height>`
- Example:

```bash
cargo run --bin atlas_proto -- 42 16 --pixels 6 --output target/atlas-debug/seed_42_16x16
```

- Notes:
  - This binary has no dedicated leaf doc yet; the source is [atlas_proto.rs](./atlas_proto.rs).
  - The output path is a directory, not a single PNG file.

## atlas_realization_chunk_topdown_preview

- Purpose: render a single PNG with same-scale top-down panels for atlas biome classification, realization controls, and final realized chunk blocks.
- Parameters:
  - positional: `<seed>`
  - optional: `--center-x <i32>` or `--chunk-x <i32>`, `--center-z <i32>` or `--chunk-z <i32>`, `--radius <i32>`, `--min-y-chunk <i32>`, `--max-y-chunk <i32>`, `--pixels-per-block <u32>`, `--output <path>`
- Defaults:
  - center `= (0, 0)`
  - `--radius 0`
  - `--min-y-chunk -2`
  - `--max-y-chunk 3`
  - `--pixels-per-block 2`
- Example:

```bash
cargo run --bin atlas_realization_chunk_topdown_preview -- 42 --center-x 41 --center-z 25 --radius 1 --pixels-per-block 1 --output target/atlas-realization-chunk-topdown-preview/seed_42_compare.png
```

- Notes:
  - This is the most direct tool for seeing how one footprint changes from atlas semantics to realization controls to final voxel surface.
  - See [atlas_realization_chunk_topdown_preview.md](./atlas_realization_chunk_topdown_preview.md).

## chunk_preview

- Purpose: render an offscreen quarter-view preview for chunks.
- Parameters:
  - source: `<seed>` or `--world-dir <path>`
  - optional: `--stage <full|prototype|hydrology>`, `--center-x <i32>`, `--center-z <i32>`, `--radius <i32>`, `--quarter-turns <u8>`, `--width <u32>`, `--height <u32>`, `--min-y-chunk <i32>`, `--max-y-chunk <i32>`, `--lod-blocks <u8>`, `--output <path>`
- Defaults:
  - center `= (0, 0)` unless a created-world manifest supplies a default preview center
  - `--radius 4`
  - `--quarter-turns 0`
  - `--width 1600`
  - `--height 900`
  - `--min-y-chunk -2`
  - `--max-y-chunk 3`
  - `--lod-blocks 1`
  - `--stage full`
- Example:

```bash
cargo run --bin chunk_preview -- 42 --stage prototype --center-x 4 --center-z -3 --radius 2 --output target/chunk-preview/prototype.png
```

- Notes:
  - Right now the reliable direct-seed paths are `--stage prototype` and `--stage hydrology`.
  - `--stage hydrology` is the best direct-seed path when you want visible carried waterways before full voxelization exists.
  - Direct-seed `full` preview still depends on `generate_chunk(...)`, which is currently a generation `todo!`.
  - `--lod-blocks > 1` currently depends on `sample_chunk_surface_lod(...)`, which is also still a generation `todo!`.
  - Created-world `full` preview can work if you already have a valid dumped world directory.
  - See [chunk_preview.md](./chunk_preview.md).

## chunk_topdown_preview

- Purpose: render an exact top-down PNG from realized block columns.
- Parameters:
  - source: `<seed>` or `--world-dir <path>`
  - optional: `--center-x <i32>` or `--chunk-x <i32>`, `--center-z <i32>` or `--chunk-z <i32>`, `--radius <i32>`, `--min-y-chunk <i32>`, `--max-y-chunk <i32>`, `--pixels-per-block <u32>`, `--output <path>`
- Defaults:
  - center `= (0, 0)`
  - `--radius 0`
  - `--min-y-chunk -2`
  - `--max-y-chunk 3`
  - `--pixels-per-block 6`
- Example:

```bash
cargo run --bin chunk_topdown_preview -- --world-dir <existing-world-dir> --center-x 0 --center-z 0 --radius 1 --output target/chunk-topdown/topdown.png
```

- Notes:
  - Direct-seed mode currently depends on `generate_chunk(...)`, so it is not the recommended path today.
  - If you already have a valid created-world dump, `--world-dir` is the exact realized-data path.
  - See [chunk_topdown_preview.md](./chunk_topdown_preview.md).

## graph_voronoi_preview

- Purpose: render 4K top-down PNG previews for the graph-first Voronoi macro graph stage and current site-field maps.
- Parameters:
  - positional: `<seed> <center-x> <center-z>` where center coordinates are world-block coordinates
  - optional: `--width <u32>`, `--height <u32>`, `--world-span-blocks <i32>`, `--region-size-blocks <i32>`, `--site-spacing-blocks <i32>`, `--stage graph_voronoi`, `--mode <all|identity|temperature|hydration|humidity|continentality|elevation|ruggedness>`, `--output <path>`
- Defaults:
  - `--width 3840`
  - `--height 2160`
  - `--world-span-blocks 32768`
  - `--region-size-blocks DEFAULT_GRAPH_REGION_SIZE_BLOCKS`
  - `--site-spacing-blocks DEFAULT_SITE_SPACING_BLOCKS`
  - `--stage graph_voronoi`
  - `--mode identity`
- Example:

```bash
cargo run --bin graph_voronoi_preview -- 42 0 0
```

```bash
cargo run --bin graph_voronoi_preview -- 42 0 0 --mode all --output target/graph-voronoi-preview/s42_maps
```

- Notes:
  - `--mode identity` preserves the existing single-PNG graph ownership preview.
  - Default single-mode filenames are short: `target/graph-voronoi-preview/s<seed>_x<center-x>_z<center-z>_<mode>.png`.
  - Default `--mode all` output is a short directory: `target/graph-voronoi-preview/s<seed>_x<center-x>_z<center-z>/`, containing `identity.png`, `temperature.png`, `hydration.png`, `continentality.png`, `elevation.png`, and `ruggedness.png`.
  - In single mode, an explicit `--output <path>.png` is treated as the target PNG path and preserved exactly; a path without an extension is treated as an output directory.
  - Width, height, world span, site spacing, stage, and generator version stay in PNG metadata rather than default filenames.
  - Each output PNG has a compact legend overlay; field maps show a gradient bar and endpoint labels, while identity mode only shows a small header.
  - The same header is embedded in the PNG `new-world-preview-header` iTXt metadata chunk, including mode and map name.
  - Graph construction uses `world::generation::graph::generate_voronoi_graph_patch(...)`; the binary only owns image sampling and PNG output.
  - Field-map modes read the smoothed `VoronoiSite::base_fields` values produced by the graph base-field stage.
  - See [graph_voronoi_preview.md](./graph_voronoi_preview.md).

## macro_map_preview

- Purpose: render one top-down PNG composite for the graph-first macro map stage, showing ocean/lake separation, sandy coast, inland elevation, white peaks, faint base Voronoi graph edges, and ridge/fault/coast candidate edge overlays.
- Parameters:
  - positional: `<seed> <center-x> <center-z>` where center coordinates are world-block coordinates
  - optional: `--width <u32>`, `--height <u32>`, `--world-span-blocks <i32>`, `--region-size-blocks <i32>`, `--site-spacing-blocks <i32>`, `--stage macro_map`, `--output <path>`
- Defaults:
  - `--width 3840`
  - `--height 2160`
  - `--world-span-blocks 32768`
  - `--region-size-blocks DEFAULT_GRAPH_REGION_SIZE_BLOCKS`
  - `--site-spacing-blocks DEFAULT_SITE_SPACING_BLOCKS`
  - `--stage macro_map`
  - output `target/macro-map-preview/s<seed>_x<center-x>_z<center-z>.png`
- Example:

```bash
cargo run --bin macro_map_preview -- 42 0 0
```

```bash
cargo run --bin macro_map_preview -- 42 0 0 --width 640 --height 360 --output target/macro-map-preview/smoke.png
```

- Notes:
  - Default filenames stay short; width, height, world span, graph sizing, stage, generator version, and source notes are written to the PNG `new-world-preview-header` iTXt metadata chunk.
  - An explicit `--output <path>.png` is treated as the target PNG path and preserved exactly; a path without an extension is treated as an output directory.
  - The binary builds a Voronoi graph patch, then calls `new_world::world::generation::generate_macro_map(&patch, MacroMapConfig::new(...))`.
  - The fill layer is nearest-site diagnostic coloring, while the base and highlighted edge overlays are drawn from actual graph corner segments (`edge.corners` / `VoronoiCorner.position`), so the visible color boundary and overlay line can differ.
  - See [macro_map_preview.md](./macro_map_preview.md).

## realization_field_preview

- Purpose: render a top-down preview of the public generation realization / prototype-control field.
- Parameters:
  - positional: `<seed>`
  - optional: `--center-x <i32>` or `--chunk-x <i32>`, `--center-z <i32>` or `--chunk-z <i32>`, `--radius <i32>`, `--blocks-per-pixel <u32>`, `--mode <composite|biome|archetype|flatness|relief|uplift|wetness|ridge|terrace|corridor>`, `--output <path>`
- Defaults:
  - center `= (0, 0)`
  - `--radius 8`
  - `--blocks-per-pixel 8`
  - `--mode composite`
- Example:

```bash
cargo run --bin realization_field_preview -- 42 --center-x 0 --center-z 0 --radius 8 --mode composite --output target/realization-field/composite.png
```

- Notes:
  - Good middle step between `region_topdown_preview` and exact realized chunk previews.
  - See [realization_field_preview.md](./realization_field_preview.md).

## region_topdown_preview

- Purpose: render a top-down preview from atlas-owned region classification rather than realized blocks.
- Parameters:
  - positional: `<seed>`
  - optional: `--center-x <i32>` or `--chunk-x <i32>`, `--center-z <i32>` or `--chunk-z <i32>`, `--radius <i32>`, `--blocks-per-pixel <u32>`, `--output <path>`
- Defaults:
  - center `= (0, 0)`
  - `--radius 8`
  - `--blocks-per-pixel 8`
- Example:

```bash
cargo run --bin region_topdown_preview -- 42 --center-x 0 --center-z 0 --radius 8 --output target/region-topdown/seed_42.png
```

- Notes:
  - Useful for inspecting biome / archetype ownership before full realization is active.
  - See [region_topdown_preview.md](./region_topdown_preview.md).

## terrain_corridor

- Purpose: search for a large-scale ocean-to-mountain corridor and chart its height progression.
- Parameters:
  - positional: `<seed>`
  - optional: `--radius-cells <i32>`, `--min-length-cells <u32>`, `--max-length-cells <u32>`, `--output <path>`
- Defaults:
  - `--radius-cells 64`
  - `--min-length-cells 10`
  - `--max-length-cells 18`
  - `--output target/terrain-corridor`
- Example:

```bash
cargo run --bin terrain_corridor -- 42 --radius-cells 64 --min-length-cells 10 --max-length-cells 18 --output target/terrain-corridor
```

- Notes:
  - The search is currently axis-aligned at atlas scale.
  - The tool prints a suggested `chunk_preview` command for midpoint inspection.
  - See [terrain_corridor.md](./terrain_corridor.md).

## terrain_find

- Purpose: find chunk candidates by launch archetype and optional meso preferences.
- Parameters:
  - catalog-only mode: `--list-archetypes` or `--list-meso`
  - search mode positional: `<seed>`
  - search mode optional: `--origin-cell-x <i32>`, `--origin-cell-z <i32>`, `--search-radius-cells <i32>`, `--chunk-step <u32>`, `--top <usize>`, repeated `--archetype <key>`, repeated `--meso <key>`, `--preview-rank <usize>`, `--preview-radius <i32>`, `--preview-width <u32>`, `--preview-height <u32>`, `--preview-quarter-turns <u8>`, `--preview-output <path>`, `--render-preview`
- Defaults:
  - origin cell `= (0, 0)`
  - `--search-radius-cells 16`
  - `--chunk-step 1`
  - `--top 8`
  - `--preview-rank 1`
  - `--preview-radius 4`
  - `--preview-width 1600`
  - `--preview-height 900`
  - `--preview-quarter-turns 0`
- Examples:

```bash
cargo run --bin terrain_find -- --list-archetypes
```

```bash
cargo run --bin terrain_find -- 42 --archetype temperate_hills --search-radius-cells 32 --chunk-step 2 --top 5
```

```bash
cargo run --bin terrain_find -- 42 --meso hill_cluster --top 5 --render-preview --preview-output target/terrain-find/hill_cluster.png
```

- Notes:
  - Preview integration always targets `chunk_preview --stage prototype`.
  - `--preview-rank` is `1`-based.
  - See [terrain_find.md](./terrain_find.md).

## terrain_probe

- Purpose: print generation diagnostics for one chunk and one selected column.
- Parameters:
  - positional: `<seed>`
  - optional: `--chunk-x <i32>`, `--chunk-z <i32>`, `--local-x <u8>`, `--local-z <u8>`
- Defaults:
  - chunk `= (0, 0)`
  - local column `= (16, 16)` for the current `32 x 32` chunk edge
- Example:

```bash
cargo run --bin terrain_probe -- 42 --chunk-x 0 --chunk-z 0 --local-x 16 --local-z 16
```

- Notes:
  - The CLI shape is present, but the binary currently hits `probe_chunk(...)` and `probe_column(...)`, which are still generation `todo!` paths.
- See [terrain_probe.md](./terrain_probe.md).

## tree_preview

- Purpose: render five deterministic variants of one `world::tree` blueprint kind with the default tree block palette.
- Parameters:
  - positional: `<tree-kind> <preview-seed>`
  - optional: `--output <path>`, `--width <u32>`, `--height <u32>`, `--quarter-turns <u8>`
- Defaults:
  - `--width 1600`
  - `--height 1000`
  - `--quarter-turns 0`
  - `--output target/tree-preview/<tree-kind>_seed_<preview-seed>.png`
- Example:

```bash
cargo run --bin tree_preview -- jungle 42 --output target/tree-preview/jungle.png
```

- Notes:
  - This tool does not mutate `WorldCore`; it folds the generated tree voxels into temporary chunks only for meshing and offscreen rendering.
  - The preview renders five same-kind variants spaced 36 blocks apart on a temporary `grass` block plane.
  - The preview uses a lower-than-gameplay camera angle and tighter framing for clearer tree reads.
  - See [tree_preview.md](./tree_preview.md).

## world_coords

- Purpose: inspect an existing created-world manifest and print promising preview coordinates.
- Parameters:
  - positional: `<world-dir>`
  - optional: `--top <usize>`
- Defaults:
  - `--top 16`
- Example:

```bash
cargo run --bin world_coords -- <existing-world-dir> --top 8
```

- Notes:
  - This does not regenerate terrain; it only reads `manifest.toml`.
  - See [world_coords.md](./world_coords.md).

## world_create

- Purpose: generate a bounded created-world dump on disk.
- Parameters:
  - positional: `<seed>`
  - optional: `--center-x <i32>`, `--center-z <i32>`, `--radius <i32>`, `--min-y-chunk <i32>`, `--max-y-chunk <i32>`, `--output <path>`
- Defaults:
  - center `= (0, 0)`
  - `--radius 16`
  - `--min-y-chunk -2`
  - `--max-y-chunk 3`
  - `--output target/world-create/seed_<seed>_cx<center_x>_cz<center_z>_r<radius>`
- Example:

```bash
cargo run --bin world_create -- 42 --center-x 0 --center-z 0 --radius 16 --output target/world-create/seed_42_demo
```

- Notes:
  - The CLI is in place, but direct world creation still depends on `generate_chunk(...)`, which is currently a generation `todo!` path.
  - See [world_create.md](./world_create.md).

## Cross-Tool Suggestions

- Use `region_topdown_preview` first when you want biome / archetype ownership across a broad seed window.
- Use `realization_field_preview` next when you want the generation control-field shape rather than semantic ids.
- Use `atlas_realization_chunk_topdown_preview` when you want atlas, realization, and final realized chunk output aligned in one same-scale image.
- Use `terrain_find` when you want candidate chunk coordinates for a specific launch archetype or meso flavor.
- Use `chunk_preview --stage prototype` when you want the post-meso terrain form without late hydrology.
- Use `chunk_preview --stage hydrology` when you want the current late carved surface plus visible waterways from the direct-seed path.
- Use `chunk_topdown_preview --world-dir ...` when you already have a valid created-world dump and need exact realized block-column inspection.
