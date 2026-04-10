# meta

## Role

- Define immutable world identity and version metadata shared by generation, storage, and world-core code.

## Responsibilities

- Define `WorldMeta`
- Own `seed`
- Own `world_version`, `generator_version`, and `save_format_version`
- Provide compatibility/version boundaries for generation and storage behavior

## Owned Data

### `WorldMeta`

- `seed`
- `world_version`
- `generator_version`
- `save_format_version`

## Invariants

1. A loaded world has exactly one authoritative `WorldMeta`.
2. `seed` is stable for the lifetime of the world instance.
3. `generator_version` must change when deterministic chunk realization changes in a way that affects generated blocks.
4. `save_format_version` must change when serialized chunk format compatibility changes.

## Current Notes

- `generator_version = 4` corresponds to the current profile-driven atlas relief pass with:
  - fixed sea level at world `y = 0`
  - generation-side terrain profile resolution (`DeepOcean`, `Shelf`, `Coast`, `Plain`, `Upland`, `Ridge`)
  - solid `stone` terrain from world `y = -256` through profile-shaped `surface_y`
  - air above the surface, including negative-height sea basins
  - no water, soil, sand, snow, vegetation, or ecology placement yet
- `save_format_version` is still `1`.
