# chunk_topdown_preview

## Role

- Render a top-down `xz` PNG from exact realized chunk data.
- Support both direct seed generation and rendering from a previously created world dump.

## Inputs

- either `seed` or `--world-dir <path>`
- preview center in chunk coordinates
- `--chunk-x` / `--chunk-z` aliases for direct chunk targeting
- horizontal chunk radius
- vertical chunk bounds
- pixels per block
- output image path

## Outputs

- a PNG image where each world block column becomes one colored top-down cell
- a stdout diagnostic summary for the same realized data and its chunk meshes
  - visible top-block counts
  - columns containing any `water`
  - columns where `water` is the top-visible block
  - total water-block depth statistics inside the scanned volume
  - chunk-mesh water-face counts
  - a coarse generation-vs-meshing-vs-renderer hint based on those counts

## Current Flow

1. Resolve the preview source.
2. Load or generate the exact requested chunk window.
3. For each `world (x, z)` column, scan from the requested max `y` down to the min `y`.
4. Record the topmost non-air block in that column.
5. Color the cell with a diagnostic material palette plus relief-based brightness.
   - `snow` keeps a preview-only white override so frozen terrain does not read as gray stone in debug images.
6. Draw subtle per-cell borders so flat areas remain readable.
7. Save the PNG to disk.
8. Print column and mesh debug summaries for the scanned window.

## Exactness Notes

- In direct-seed mode, this tool uses the same `world::generation::generate_chunk(...)` path as `world_create`.
- In created-world mode, it reads the persisted chunk `.bin` payloads and scans the actual loaded chunk contents.
- The current top-down sampling and diagnostic color rules are mirrored by the shared `world::topdown` helper so app minimap overlays can match this preview style.
- That means the top-down geometry is exact for the chosen projection rule: the image represents the topmost non-air block found in each `xz` column inside the requested vertical window.
- The colors are intentionally diagnostic and are not meant to match final renderer shading or texture sampling.
- The stdout counters are also exact for the chosen window because they are computed from the same realized `ChunkData` and from meshes built through `world::meshing::build_chunk_mesh(...)`.

## Visibility Notes

- Borders are always drawn between block cells when `pixels-per-block >= 2`.
- Borders become slightly stronger where adjacent cells differ in top block or top `y`, which makes step changes and river cuts easier to read.

## Diagnostic Notes

- If the summary reports zero `water` columns and zero water faces, the issue is upstream of rendering and points at generation.
- If the summary reports `water` columns but zero water faces, inspect world meshing or block render metadata.
- If the summary reports both visible `water` columns and water faces, but the live game view still hides them, the remaining suspect is renderer presentation or shading rather than chunk generation.
