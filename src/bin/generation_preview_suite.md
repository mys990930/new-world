# generation_preview_suite

## Role

- Run the main graph-first generation preview binaries in a fixed order.
- Collect their PNG outputs into one directory with short ordered filenames.
- Keep this binary as orchestration only. It does not duplicate preview rendering code or change
  terrain generation policy.

## Inputs

```bash
cargo run --bin generation_preview_suite -- <seed> [options]
```

Options:

- `--center-chunk-x <i32>` or `--cx <i32>`: center chunk X, default `0`.
- `--center-chunk-z <i32>` or `--cz <i32>`: center chunk Z, default `0`.
- `--radius <i32>` or `--r <i32>`: non-negative chunk radius for zoom/pixelize/heightfield previews, default `8`.
  `0` is allowed for one-chunk smoke runs.
- `--output <path>`: output directory, default
  `target/generation-preview-suite/s<seed>_cx<cx>_cz<cz>_r<r>`.
- `--overview-width <u32>` / `--overview-height <u32>`: graph/macro overview image size, default
  `3840 x 2160`.
- `--zoom-width <u32>` / `--zoom-height <u32>`: macro zoom and pixelize image size, default
  `1280 x 720`.
- `--heightfield-width <u32>` / `--heightfield-height <u32>`: heightfield image size, default
  `1280 x 720`.
- `--contour-step <i32>`: macro field contour step in blocks, default `8`.

## Output

The suite writes eight files:

```text
01_graph_cont.png
02_graph_elev.png
03_macro_map.png
04_biome_map.png
05_macro_combined.png
06_macro_zoom.png
07_pixelize.png
08_heightfield.png
```

It prints each child command, elapsed time, and final output path. If any child preview fails, the
suite fails immediately.

## Coordinate Contract

- The suite accepts chunk coordinates.
- Overview binaries that expect world-block centers receive:

```text
world_x = center_chunk_x * CHUNK_EDGE + CHUNK_EDGE / 2
world_z = center_chunk_z * CHUNK_EDGE + CHUNK_EDGE / 2
```

- `macro_map_preview`, `macro_field_preview`, `pixelize_preview`, and `heightfield_preview` receive chunk coordinates directly.
- Graph and biome map overview children receive the converted world-block center.
- Steps 03 and 05 use a fixed large macro overview radius; step 06 receives the suite radius exactly.

## Child Binaries

The suite first tries to run sibling executables from the same directory as itself. This is the
normal release-build path after the preview binaries have been built.

If a sibling executable is missing, it falls back to:

```text
cargo run --bin <child> -- ...
```

This keeps local development convenient while still making release runs avoid nested Cargo when the
preview binaries already exist.

## Sequence

1. `graph_voronoi_preview --mode continentality`
2. `graph_voronoi_preview --mode elevation`
3. `macro_map_preview`
4. `biome_map_preview`
5. `macro_field_preview <seed> <cx> <cz> <overview-r> --channel combined --contours --contour-step 8`
6. `macro_field_preview <seed> <cx> <cz> <r> --channel combined --contours --contour-step 8`
7. `pixelize_preview <seed> <cx> <cz> <r>`
8. `heightfield_preview <seed> <cx> <cz> --chunk-radius <r>`

## Examples

Default r8 suite:

```bash
cargo run --bin generation_preview_suite -- 42 --center-chunk-x -70 --center-chunk-z 0 --radius 8 --output target/generation-preview-suite/s42_cx-70_cz0_r8
```

Small smoke:

```bash
cargo run --bin generation_preview_suite -- 42 --cx 0 --cz 0 --radius 1 --overview-width 160 --overview-height 90 --zoom-width 160 --zoom-height 90 --heightfield-width 240 --heightfield-height 135 --output target/generation-preview-suite/smoke
```
