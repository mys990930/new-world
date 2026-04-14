# chunk_topdown_preview

## Role

- Render a top-down `xz` PNG from exact realized chunk data.
- Support both direct seed generation and rendering from a previously baked world dump.

## Inputs

- either `seed` or `--world-dir <path>`
- preview center in chunk coordinates
- horizontal chunk radius
- vertical chunk bounds
- pixels per block
- output image path

## Outputs

- a PNG image where each world block column becomes one colored top-down cell

## Current Flow

1. Resolve the preview source.
2. Load or generate the exact requested chunk window.
3. For each `world (x, z)` column, scan from the requested max `y` down to the min `y`.
4. Record the topmost non-air block in that column.
5. Color the cell with a diagnostic material palette plus relief-based brightness.
6. Draw subtle per-cell borders so flat areas remain readable.
7. Save the PNG to disk.

## Exactness Notes

- In direct-seed mode, this tool uses the same `world::generation::generate_chunk(...)` path as `world_bake`.
- In baked-world mode, it reads the persisted chunk `.bin` payloads and scans the actual loaded chunk contents.
- That means the top-down geometry is exact for the chosen projection rule: the image represents the topmost non-air block found in each `xz` column inside the requested vertical window.
- The colors are intentionally diagnostic and are not meant to match final renderer shading or texture sampling.

## Visibility Notes

- Borders are always drawn between block cells when `pixels-per-block >= 2`.
- Borders become slightly stronger where adjacent cells differ in top block or top `y`, which makes step changes and river cuts easier to read.
