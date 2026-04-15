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

- `generator_version = 11` corresponds to the current profile-driven atlas relief and first-pass material layering with:
  - fixed sea level at world `y = 0`
  - generation-side terrain profile resolution (`DeepOcean`, `Shelf`, `Coast`, `Plain`, `Upland`, `Ridge`)
  - blended profile surface evaluation so neighboring macro profiles ease into one another instead of producing abrupt height seams
  - a stone core from world `y = -256` through `surface_y - random(8..=16)`
  - atlas-informed sediment/topsoil selection above the stone core (`mud`, `sand`, `gravel`, `snow`, `dirt`, `grass`)
  - coast-first fill classification near sea level so beach sand remains visible even when river signals are nearby
  - snow cover gated by actual coldness, so `polar_factor` still freezes terrain outright but warm alpine ridges no longer turn into `snow` just because mountain form is strong
  - smoother river floodplain/channel carving on top of the blended terrain surface, currently only for columns already resolved as river-bearing land
  - automatic sea water fill only for `DeepOcean` / `Shelf` columns
  - coast columns clamped to at least sea level so beaches do not turn into scattered sea-filled pockets
  - inland river water fill above sea level when a carved channel resolves a `water_top_y`
  - atlas distance fields that stay "far" instead of collapsing to zero when a tiny sampled neighborhood has no local ocean or river source
  - a padded generation atlas neighborhood so local chunk realization sees more macro context than a bare 2x2 atlas slice
  - stronger mountain/ridge promotion and more severe upland/ridge relief shaping for prototype terrain inspection, including more aggressive ridge cliffs and couloirs
  - no vegetation or ecology placement yet
- `save_format_version` is still `1`.
