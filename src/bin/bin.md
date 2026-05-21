# bin

## Role

- Index the standalone binaries under `src/bin`.
- Summarize what each binary does, which parameters it accepts, and which command shape is recommended right now.
- Keep this index focused on active tools only. Removed legacy atlas / realization / terrain-search preview binaries are intentionally not listed here.
- Active preview images include a small compass overlay. Topdown previews use macro-field/world
  orientation: image top is north (N), right is east (E), bottom is south (S), and left is west (W).
  Quarter-view previews may project the compass through their current camera/quarter transform; see
  the binary-specific docs.
- Preview binaries may share bin-local helpers under `src/bin/common` for drawing primitives,
  compass overlays, and thin graph-first preview setup. These helpers preserve each binary's CLI,
  filenames, metadata keys, and stage-specific rendering meaning; world generation semantics remain
  owned by `world::generation`.

## Current Index

| Binary | Purpose | Current status |
| --- | --- | --- |
| `biome_cell_inspector` | Responsive HTML/SVG cell inspector for resolved biome cells plus lake/river overlays | Works; reads graph macro map biomes, macro lake edge classes, and selected hydrology output |
| `biome_map_preview` | Top-down graph-first biome map preview, one color per resolved Voronoi cell | Works; reads `GraphMacroMap.biomes` from the core biome classifier output |
| `chunk_preview` | Quarter-view chunk preview | Recommended in `--stage prototype` or `--stage hydrology`; direct-seed `full` and `--lod-blocks > 1` are currently blocked by generation TODOs |
| `chunk_topdown_preview` | Exact top-down realized block-column preview | Works with an existing created-world dump; direct-seed mode currently depends on `generate_chunk(...)` TODO |
| `generation_preview_suite` | Orchestrates the key graph-first preview binaries into one ordered folder | Works as a thin child-process suite |
| `graph_voronoi_preview` | 4K top-down graph-first Voronoi macro graph and site-field preview maps | Works |
| `heightfield_preview` | Quarter-view graph-first heightfield column preview from macro field cache | Works |
| `macro_field_preview` | Top-down graph-first macro field rasterization channel preview | Works |
| `macro_map_preview` | Top-down graph-first macro map composite with ocean/land/elevation and selected guide overlays | Works |
| `meso_preview` | Top-down isolated meso-feature preview over a flat baseline | Works for explicit seed / coordinate windows |
| `new-world-textmode` | Continuously refreshed console grid for fixed tick time/weather/surface/ecology events over the ECS `3x3` active chunk scope | Works as the first textmode simulation slice |
| `pixelize_preview` | Top-down graph-first chunk pixelize preview, one resolved column per world block by default | Works with the current core `pixelize` API export |
| `surface_plan_preview` | Quarter-view graph-first surface/material plan preview from heightfield columns | Works with the current core `surface_plan` API export |
| `terrain_probe` | Per-chunk / per-column generation probe dump | Currently blocked by `probe_chunk(...)` and `probe_column(...)` TODO |
| `tree_preview` | Quarter-view preview of five generated variants for one climate tree blueprint | Works |
| `world_create` | Generate and persist a created-world dump | Works through the graph-first launch voxel fill path |

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

## biome_map_preview

- Purpose: render a top-down PNG preview that colors each graph Voronoi cell by resolved biome.
- Parameters:
  - positional: `<seed> <center-x> <center-z>` where center coordinates are world-block coordinates
  - optional: `--width <u32>`, `--height <u32>`, `--world-span-blocks <i32>`, `--region-size-blocks <i32>`, `--site-spacing-blocks <i32>`, `--land-bias <f32>`, `--stage biome_map`, `--output <path>`
- Defaults:
  - `--width 3840`
  - `--height 2160`
  - `--world-span-blocks 32768`
  - `--stage biome_map`
- Example:

```bash
cargo run --bin biome_map_preview -- 42 0 0 --width 640 --height 360 --output target/biome-map-preview/smoke.png
```

- Notes:
  - The binary builds a Voronoi graph patch and macro map through public generation APIs, then colors
    the `GraphBiomeCell` values exposed through `GraphMacroMap.biomes`.
  - It does not edit or duplicate core biome classifier policy.
  - See [biome_map_preview.md](./biome_map_preview.md).

## biome_cell_inspector

- Purpose: write a responsive HTML/SVG inspector where hovering or clicking a Voronoi cell shows
  that cell's resolved biome, final context values, macro surface kind, lake overlay counts, and
  selected river overlay data.
- Parameters:
  - positional: `<seed> <center-x> <center-z>` where center coordinates are world-block coordinates
  - optional: `--width <u32>`, `--height <u32>`, `--world-span-blocks <i32>`, `--region-size-blocks <i32>`,
    `--site-spacing-blocks <i32>`, `--land-bias <f32>`, `--stage biome_cell_inspector`, `--output <path>`
- Defaults:
  - `--width 1400`
  - `--height 900`
  - `--world-span-blocks 32768`
  - `--stage biome_cell_inspector`
- Example:

```bash
cargo run --bin biome_cell_inspector -- 42 0 0 --output target/biome-cell-inspector/s42.html
```

- Notes:
  - Cell colors come from resolved `GraphMacroMap.biomes`, matching the detailed biome palette.
  - Lake edge and selected river overlays come from the macro-map/hydrology path rather than from
    duplicated preview-only rules.
  - See [biome_cell_inspector.md](./biome_cell_inspector.md).

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

- Purpose: render 4K top-down PNG previews for the graph-first Voronoi macro graph stage, current site-field maps, and resolved biome mode.
- Parameters:
  - positional: `<seed> <center-x> <center-z>` where center coordinates are world-block coordinates
  - optional: `--width <u32>`, `--height <u32>`, `--world-span-blocks <i32>`, `--region-size-blocks <i32>`, `--site-spacing-blocks <i32>`, `--stage graph_voronoi`, `--mode <all|identity|temperature|hydration|humidity|biome|continentality|elevation|ruggedness>`, `--output <path>`
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
  - Field-map modes read resolved `GraphMacroMap.biomes` context when available and fall back to
    smoothed `VoronoiSite::base_fields` only as a diagnostic fallback.
  - Biome mode colors the resolved final `GraphBiomeKind`; the core classifier owns how final
    temperature, hydration, continentality, elevation, water role, coastness, mountainness, and
    ruggedness influence that result.
  - `--mode biome` colors every detailed `GraphBiomeKind` variant with a distinct palette and
    includes a compact two-column legend.
  - See [graph_voronoi_preview.md](./graph_voronoi_preview.md).

## generation_preview_suite

- Purpose: run the main graph-first preview sequence and collect the PNGs into one directory.
- Parameters:
  - positional: `<seed>`
  - optional: `--center-chunk-x <i32>` / `--cx <i32>`, `--center-chunk-z <i32>` / `--cz <i32>`,
    `--radius <i32>` / `--r <i32>`, `--output <path>`, `--overview-width <u32>`,
    `--overview-height <u32>`, `--zoom-width <u32>`, `--zoom-height <u32>`,
    `--heightfield-width <u32>`, `--heightfield-height <u32>`, `--contour-step <i32>`
- Defaults:
  - center chunk `= (0, 0)`
  - `--radius 8`
  - `--radius 0` is allowed for one-chunk smoke runs; the macro-field zoom child receives `1`
    internally because it needs a nonzero world span.
  - overview images `= 3840 x 2160`
  - zoom/pixelize images `= 1280 x 720`
  - heightfield image `= 1280 x 720`
  - `--contour-step 8`
  - output directory `target/generation-preview-suite/s<seed>_cx<cx>_cz<cz>_r<r>`
- Output files:
  - `01_graph_cont.png`
  - `02_graph_elev.png`
  - `03_macro_map.png`
  - `04_biome_map.png`
  - `05_macro_combined.png`
  - `06_macro_zoom.png`
  - `07_pixelize.png`
  - `08_heightfield.png`
- Example:

```bash
cargo run --bin generation_preview_suite -- 42 --center-chunk-x -70 --center-chunk-z 0 --radius 8 --output target/generation-preview-suite/s42_cx-70_cz0_r8
```

- Notes:
  - The suite accepts chunk coordinates, converts them to world-block centers for graph/biome
    overview binaries, and forwards chunk center/radius directly to macro-map and chunk-footprint previews.
  - It reuses existing preview binaries as child processes and does not own terrain/rendering policy.
  - See [generation_preview_suite.md](./generation_preview_suite.md).

## heightfield_preview

- Purpose: render a quarter-view diagnostic preview for graph-first heightfield columns.
- Parameters:
  - positional: `<seed> <center-x> <center-z>` where center coordinates are chunk coordinates by default
  - optional: `--width <u32>`, `--height <u32>`, `--world-span-blocks <i32>`, `--chunk-radius <i32>`, `--columns-x <u32>`,
    `--columns-z <u32>`, `--region-size-blocks <i32>`, `--site-spacing-blocks <i32>`,
    `--land-bias <f32>`, `--quarter-turns <u8>`, `--world-center`, `--stage heightfield`,
    `--output <path>`
- Defaults:
  - `--width 1280`
  - `--height 720`
  - `--world-span-blocks 8192`
  - `--columns-x 768` in free-window mode
  - `--chunk-radius` mode uses `32` columns per covered chunk by default
  - `--columns-z` derived from image aspect
- Example:

```bash
cargo run --release --bin heightfield_preview -- 42 0 0 --chunk-radius 8 --quarter-turns 0 --output target/heightfield-preview/heightfield.png
```

- Notes:
  - This is not final `ChunkData` voxel fill. It converts `MacroFieldTile` to `HeightfieldTile`,
    then renders diagnostic voxelized columns.
  - Positional center is chunk-based so it is consistent with `--chunk-radius`; `--world-center` is
    only a compatibility path for old world-block invocations.
  - The compass follows the current quarter-view projection. Per-block face outlines and integer
    side-step guides are not rendered; scale context comes from filled faces, the player diagnostic
    cube, and world/grid overlays.
  - Horizontal density is expressed as direct column count. Free-window mode defaults to `768` X
    columns; chunk-radius mode defaults to `32` columns per chunk on each axis, matching one
    column per world block for the current `32` block chunk edge. Heightfield Y relief is resolved
    in the macro/heightfield block-domain, and preview rendering keeps block primitives visually
    cubic instead of applying an extra vertical scale.
  - Heightfield terrace resolve uses integer contour steps with a one-block raw minimum gap for
    both general land and river corridors; the river override structure remains available for later
    tuning.
  - Auto output names include quarter and chunk radius suffixes such as `s42_cx0_cz0_q0_r4.png`.
    Explicit `--output` paths are respected exactly.
  - Meso feature is currently stubbed to zero. Perlin micro relief is on by default; `--perlin`
    remains accepted as a backward-compatible alias.
  - See [heightfield_preview.md](./heightfield_preview.md).

## macro_field_preview

- Purpose: render top-down PNG previews for graph-first macro field rasterization.
- Parameters:
  - positional: `<seed> <cx> <cz> <r>` where `cx/cz` are chunk coordinates and `r` is inclusive chunk radius
  - optional: `--width <u32>`, `--height <u32>`, `--region-size-blocks <i32>`,
    `--site-spacing-blocks <i32>`, `--land-bias <f32>`, `--stage macro_field`,
    `--channel <all|macro|mask|ridge|river|combined|lit|contour>`, `--contours`, `--contour-step <blocks>`,
    `--contour-major-every <n>`, `--output <path>`
- Defaults:
  - `--width 3840`
  - `--height 2160`
  - `--channel lit`
- Example:

```bash
cargo run --release --bin macro_field_preview -- 42 0 0 8 --width 1280 --height 720 --channel combined --contours --output target/macro-field-preview/combined.png
```

- Notes:
  - `combined` uses the subtle pre-Perlin terrain ramp.
  - `lit` is white-material broad hillshade from combined macro height.
  - `contour` visualizes heightfield-pre-step contour bands from the same combined macro height domain.
  - `river` and `combined` should show flat-bottom river valleys with flow-scaled shoulders, not
    narrow V cuts.
  - See [macro_field_preview.md](./macro_field_preview.md).

## pixelize_preview

- Purpose: render a top-down PNG for the graph-first pixelize stage over an inclusive square chunk
  footprint.
- Parameters:
  - positional: `<seed> <cx> <cz> <r>` where `cx/cz` are center chunk coordinates and `r` is the
    inclusive square chunk radius
  - optional: `--width <u32>`, `--height <u32>`, `--region-size-blocks <i32>`,
    `--site-spacing-blocks <i32>`, `--land-bias <f32>`, `--stage pixelize`, `--output <path>`
- Defaults:
  - output dimensions default to `(2r + 1) * CHUNK_EDGE` on each axis, so the default image has one
    output pixel per pixelized world-block column
  - default output path is `target/pixelize-preview/s<seed>_cx<cx>_cz<cz>_r<r>.png`
- Example:

```bash
cargo run --bin pixelize_preview -- 42 0 0 1 --output target/pixelize-preview/smoke.png
```

- Notes:
  - The binary builds graph, macro map, hydrology, noisy boundary, and macro field inputs through
    public generation APIs, then calls the core pixelize API.
  - `--width` and `--height` scale the already pixelized chunk area for display; the underlying
    data remains one column per world block.
  - Depends on the core `PixelizeConfig`, `generate_pixelized_chunk_area`, `PixelizedChunkArea`,
    and `PixelizedColumn` exports.
  - See [pixelize_preview.md](./pixelize_preview.md).

## macro_map_preview

- Purpose: render one top-down PNG composite for the graph-first macro map stage, showing ocean/lake separation, coast, inland elevation, base Voronoi graph edges, hydrology, and guide overlays.
- Parameters:
  - positional: `<seed> <cx> <cz> <r>` where center coordinates are chunk coordinates and `r` is an inclusive square chunk radius
  - optional: `--width <u32>`, `--height <u32>`, `--region-size-blocks <i32>`, `--site-spacing-blocks <i32>`, `--stage macro_map`, `--output <path>`
- Defaults:
  - `--width 3840`
  - `--height 2160`
  - `--stage macro_map`
- Example:

```bash
cargo run --bin macro_map_preview -- 42 0 0 10 --width 640 --height 360 --output target/macro-map-preview/smoke.png
```

- Notes:
  - The binary builds a Voronoi graph patch, then calls `new_world::world::generation::generate_macro_map(...)`.
  - Generic RGB drawing primitives come from `src/bin/common/preview_draw.rs`; macro-map colors,
    hydrology marker policy, metadata, CLI, and generation setup remain local to this binary.
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

## new-world-textmode

- Purpose: run fixed ticks and redraw one structured simulation/world `3x3` chunk grid per real second.
- Parameters:
  - optional: `--seed <u64>`, `--seconds <u32>`, `--ticks-per-second <u32>`, `--center-chunk-x <i32>`, `--center-chunk-y <i32>`, `--center-chunk-z <i32>`
- Defaults:
  - `--seed 42`
  - `--seconds` unset, so the binary runs until `Ctrl+C`
  - `--ticks-per-second 20`
  - center chunk `= (0, 0, 0)`
- Example:

```bash
cargo run --bin new-world-textmode
```

```bash
cargo run --bin new-world-textmode -- --seconds 5
```

- Notes:
  - Output formatting is a binary adapter. It reads ECS chunk scope, world observers, and structured simulation events.
  - Chunk biome labels and weather input context are sampled from graph-first `GraphMacroMap.biomes`, matching `biome_cell_inspector`.
  - Weather runs through the simulation weather subsystem, applies `ChunkWeatherUpdate` records to `WorldCore`, and displays world-owned scalar state as `weather : Cloudy temp=0.62 moist=0.44 cloud=0.71 rain=0.18`.
  - The console is cleared and redrawn once per second with box-drawing grid borders.
  - The default run has no second limit; use `Ctrl+C` to stop it. `--seconds` is only for smoke tests and demos.
  - Chunk-cell labels use a fixed label width so values align line-by-line.
  - Temporary chunks are realized as empty in-memory chunks for this first slice.
  - See [new-world-textmode.md](./new-world-textmode.md).

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
  - Direct world creation now uses the graph-first launch path:
    graph/macro/hydrology/boundary/macro-field/heightfield/surface-plan/pixelize ->
    `GraphFirstVoxelPlan` -> `ChunkData`.
  - Perlin micro relief and surface/material policy are enabled for the graph-first plan; vegetation
    placement is still stubbed.
  - See [world_create.md](./world_create.md).

## Cross-Tool Suggestions

- Use `graph_voronoi_preview` to inspect graph identity and smoothed base fields.
- Use `generation_preview_suite` when you want the standard graph-first preview sequence in one
  ordered output folder.
- Use `macro_map_preview` to inspect macro ownership, coast/lake separation, hydrology, and guide overlays.
- Use `macro_field_preview` to inspect graph-derived raster fields, contours, combined macro height, and lit top-down height previews.
- Use `pixelize_preview` to inspect chunk-aligned one-column-per-block output before the heightfield
  and voxel-column realization path.
- Use `heightfield_preview` to inspect contour-guided heightfield columns before final voxel fill.
- Use `chunk_preview --stage prototype` or `--stage hydrology` when you need the older chunk-oriented diagnostic paths.
- Use `chunk_topdown_preview --world-dir ...` when you already have a valid created-world dump and need exact realized block-column inspection.
- Read topdown preview compasses as common world orientation markers: up=N and right=E. For
  quarter-view tools, read the binary docs because the compass may be projected through the view.
