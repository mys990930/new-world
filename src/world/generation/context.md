# context

## Role

- Define the small shared structs used across generation submodules.

## Responsibilities

- `GenerationPalette`
- `ColumnAtlasSample`
- `ColumnRealization`
- generation-internal fill profile and river-stage enums

## Notes

- `ColumnAtlasSample` is the generation-side, interpolated view of atlas data for a single block column.
- `ColumnAtlasSample` is also exposed through generation probe APIs so tuning work can inspect the exact bilerp inputs that shaped a column.
- `ColumnRealization` stores the carved ground `surface_y`, optional `water_top_y`, stone-core ceiling, and material fill profile for that column.
- `GenerationPalette` now maps the first-pass layered terrain materials (`grass`, `dirt`, `stone`, `sand`, `gravel`, `mud`, `snow`, `water`) from the registry in one place.
