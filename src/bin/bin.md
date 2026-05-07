# bin

## Role

- Index the standalone binaries under `src/bin`.
- Summarize what each binary does, which parameters it accepts, and which command shape is recommended right now.
- Keep this index focused on active tools only. Removed legacy atlas / realization / terrain-search preview binaries are intentionally not listed here.

## Current Index

| Binary | Purpose | Current status |
| --- | --- | --- |
| `chunk_preview` | Quarter-view chunk preview | Recommended in `--stage prototype` or `--stage hydrology`; direct-seed `full` and `--lod-blocks > 1` are currently blocked by generation TODOs |
| `chunk_topdown_preview` | Exact top-down realized block-column preview | Works with an existing created-world dump; direct-seed mode currently depends on `generate_chunk(...)` TODO |
| `graph_voronoi_preview` | 4K top-down graph-first Voronoi macro graph and site-field preview maps | Works |
| `heightfield_preview` | Quarter-view graph-first heightfield column preview from macro field cache | Works |
| `macro_field_preview` | Top-down graph-first macro field rasterization channel preview | Works |
| `macro_map_preview` | Top-down graph-first macro map composite with ocean/land/elevation and selected guide overlays | Works |
| `meso_preview` | Top-down isolated meso-feature preview over a flat baseline | Works for explicit seed / coordinate windows |
| `terrain_probe` | Per-chunk / per-column generation probe dump | Currently blocked by `probe_chunk(...)` and `probe_column(...)` TODO |
| `tree_preview` | Quarter-view preview of five generated variants for one climate tree blueprint | Works |
| `world_create` | Generate and persist a created-world dump | Currently blocked by `generate_chunk(...)` TODO |

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
  - Field-map modes read the smoothed `VoronoiSite::base_fields` values produced by the graph base-field stage.
  - See [graph_voronoi_preview.md](./graph_voronoi_preview.md).

## heightfield_preview

- Purpose: render a quarter-view diagnostic preview for graph-first heightfield columns.
- Parameters:
  - positional: `<seed> <center-x> <center-z>` where center coordinates are world-block coordinates
  - optional: `--width <u32>`, `--height <u32>`, `--world-span-blocks <i32>`, `--chunk-radius <i32>`, `--columns-x <u32>`,
    `--columns-z <u32>`, `--region-size-blocks <i32>`, `--site-spacing-blocks <i32>`,
    `--land-bias <f32>`, `--quarter-turns <u8>`, `--vertical-scale <f32>`, `--stage heightfield`,
    `--output <path>`
- Defaults:
  - `--width 1280`
  - `--height 720`
  - `--world-span-blocks 8192`
  - `--columns-x 192`
  - `--columns-z` derived from image aspect
- Example:

```bash
cargo run --release --bin heightfield_preview -- 42 0 0 --chunk-radius 32 --width 1280 --height 720 --output target/heightfield-preview/heightfield.png
```

- Notes:
  - This is not final `ChunkData` voxel fill. It converts `MacroFieldTile` to `HeightfieldTile`,
    then renders diagnostic voxelized columns.
  - Meso feature and Perlin micro relief are currently stubbed to zero.
  - See [heightfield_preview.md](./heightfield_preview.md).

## macro_field_preview

- Purpose: render top-down PNG previews for graph-first macro field rasterization.
- Parameters:
  - positional: `<seed> <center-x> <center-z>` where center coordinates are world-block coordinates
  - optional: `--width <u32>`, `--height <u32>`, `--world-span-blocks <i32>`, `--region-size-blocks <i32>`,
    `--site-spacing-blocks <i32>`, `--land-bias <f32>`, `--stage macro_field`,
    `--channel <all|macro|mask|ridge|river|combined|lit|contour>`, `--contours`, `--contour-step <blocks>`,
    `--contour-major-every <n>`, `--output <path>`
- Defaults:
  - `--width 3840`
  - `--height 2160`
  - `--world-span-blocks 32768`
  - `--channel lit`
- Example:

```bash
cargo run --release --bin macro_field_preview -- 42 0 0 --width 1280 --height 720 --channel combined --contours --output target/macro-field-preview/combined.png
```

- Notes:
  - `combined` uses the subtle pre-Perlin terrain ramp.
  - `lit` is white-material broad hillshade from combined macro height.
  - `contour` visualizes heightfield-pre-step contour bands from the same combined macro height domain.
  - See [macro_field_preview.md](./macro_field_preview.md).

## macro_map_preview

- Purpose: render one top-down PNG composite for the graph-first macro map stage, showing ocean/lake separation, coast, inland elevation, base Voronoi graph edges, hydrology, and guide overlays.
- Parameters:
  - positional: `<seed> <center-x> <center-z>` where center coordinates are world-block coordinates
  - optional: `--width <u32>`, `--height <u32>`, `--world-span-blocks <i32>`, `--region-size-blocks <i32>`, `--site-spacing-blocks <i32>`, `--stage macro_map`, `--output <path>`
- Defaults:
  - `--width 3840`
  - `--height 2160`
  - `--world-span-blocks 32768`
  - `--stage macro_map`
- Example:

```bash
cargo run --bin macro_map_preview -- 42 0 0 --width 640 --height 360 --output target/macro-map-preview/smoke.png
```

- Notes:
  - The binary builds a Voronoi graph patch, then calls `new_world::world::generation::generate_macro_map(...)`.
  - See [macro_map_preview.md](./macro_map_preview.md).

## meso_preview

- Purpose: render a dedicated meso-only preview on top of a flat plain baseline.
- Parameters:
  - positional: `<seed>`
  - optional: `--center-x <i32>`, `--center-z <i32>`, `--radius <i32>`, `--blocks-per-pixel <u32>`,
    `--feature <all|hill_cluster|shallow_basin|escarpment_band|upland_terrace|ravine|coastal_cliff_band|dune_field|crater>`,
    `--corridors <none|live>`, `--overlay <none|hill_peaks>`, `--base-height <f32>`,
    `--relief-budget <f32>`, `--contour-step <f32>`, `--output <path>`
- Example:

```bash
cargo run --bin meso_preview -- 42 --center-x -57 --center-z 93 --radius 10 --feature hill_cluster
```

- Notes:
  - This tool does not search for candidate locations; pass explicit seed coordinates.
  - See [meso_preview.md](./meso_preview.md).

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
  - See [tree_preview.md](./tree_preview.md).

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

- Use `graph_voronoi_preview` to inspect graph identity and smoothed base fields.
- Use `macro_map_preview` to inspect macro ownership, coast/lake separation, hydrology, and guide overlays.
- Use `macro_field_preview` to inspect graph-derived raster fields, contours, combined macro height, and lit top-down height previews.
- Use `heightfield_preview` to inspect contour-guided heightfield columns before final voxel fill.
- Use `chunk_preview --stage prototype` or `--stage hydrology` when you need the older chunk-oriented diagnostic paths.
- Use `chunk_topdown_preview --world-dir ...` when you already have a valid created-world dump and need exact realized block-column inspection.
