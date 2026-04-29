# scale

## Role

- Define atlas-space coordinate and hard-scale rules.

## Responsibilities

- define `AtlasCoord`
- define `AtlasArea`
- define atlas cell size in chunk, region, and metric units
- keep atlas indexing deterministic

## Hard Scale

- `1 block = 0.5m`
- `1 chunk = 32 x 32 x 32 blocks`
- `1 chunk side = 16m`
- `1 atlas cell = 8 x 8 chunk columns`
- `1 atlas cell = 256 x 256 block columns`
- `1 atlas cell = 128m x 128m`
- `1 atlas cell = 1 x 1 region`

## Invariants

1. Atlas area width and height must both be greater than zero.
2. Atlas grid indexing must be deterministic from `(x, z)`.
3. Atlas coordinates must be stable enough for chunk generation to sample them repeatedly.
