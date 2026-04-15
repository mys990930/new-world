# topdown

## Role

- own exact top-down block-column sampling rules for realized world data
- provide shared preview-color and edge-strength helpers for debug tools and app minimap overlays
- provide worker-safe snapshot-based chunk-column derivation for cached minimap rebuild jobs

## Responsibilities

- scan realized `xz` columns through a requested vertical window
- identify the top visible block, top solid block, and visible water presence for each column
- derive a relief-aware color for each sampled column from `BlockRegistry`
- expose per-cell edge-strength helpers so callers can reproduce the same outline treatment at different pixel sizes

## Non-Responsibilities

- image file encoding
- UI layout ownership
- renderer sprite emission
- gameplay interaction policy

## Owned Data

### `TopdownCell`
- `top_y`
- `block`

### `TopdownColumnScan`
- `visible`
- `top_solid`
- `top_water_y`
- `water_block_count`

### `TopdownSurfaceRange`
- `min_y`
- `max_y`

### `TopdownChunkColumnCoord`
- chunk `x`
- chunk `z`

### `TopdownChunkColumnPatch`
- chunk-column coord
- cached `TopdownColumnScan` grid for one `32x32` chunk column

### `TopdownEdge`
- `Left`
- `Top`
- `Right`
- `Bottom`

## Inputs

- `WorldCore`
- `BlockRegistry`
- `ChunkSnapshot`
- requested world-space `x/z` window in blocks
- requested world-space vertical scan range

## Outputs

- sampled `TopdownColumnScan` grid
- snapshot-derived `TopdownChunkColumnPatch`
- optional surface range for visible blocks
- per-cell RGB preview colors
- edge strengths that callers can map to outline darkening

## State Rules

- sampling uses exact realized `WorldCore` block contents only
- snapshot-based chunk-column derivation uses only the provided immutable chunk snapshots and never reaches back into `WorldCore`
- sampling never generates or loads missing chunks implicitly
- visible cells are based on the topmost non-air block in each scanned column
- top-down preview color stays diagnostic and registry-driven rather than renderer-shaded
- water may exist below the visible top block; that distinction remains available in `TopdownColumnScan`

## Invariants

- top-down helpers stay read-only
- callers may render the same sampled data at different pixel densities, but the cell color and edge-strength rules stay world-owned
- the same sampled rules should be reusable by both `chunk_topdown_preview` and app minimap overlays
- the same chunk-column color and edge rules should remain reusable whether the source is a live `WorldCore` query or a jobs-built cached patch

## Related Modules

- `core.md`
- `query.md`
- `registry.md`

## Notes

- the current color rules intentionally match the diagnostic style used by `chunk_topdown_preview` rather than final live renderer shading
- the current minimap overlay uses these helpers with atlas-backed UI sprites instead of a separate UI texture path
- the current runtime now prefers snapshot-based chunk-column rebuilds in jobs, and only uses single-column live resampling for future local patch updates after world edits
