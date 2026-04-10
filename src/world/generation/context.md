# context

## Role

- Define the small shared structs used across generation submodules.

## Responsibilities

- `GenerationPalette`
- `ColumnAtlasSample`
- `ColumnRealization`

## Notes

- `ColumnAtlasSample` is the generation-side, interpolated view of atlas data for a single block column.
- `ColumnRealization` stores the resolved `TerrainProfile` and final `surface_y` for that column.
- The current pre-material phase only needs a single terrain scaffold block, but the palette type is kept as a module boundary so later material passes can expand without reworking every call site.
