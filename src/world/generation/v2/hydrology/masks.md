# hydrology/masks

## Role

- derive hydrology masks and coarse modes for voxel/material policy after terrain and water decisions have enough context
- keep saturation, basin, water-presence, and final mode rules separate from terrain shaping

## Boundaries

- owns scalar masks and `HydrologyMode` classification
- does not carve terrain, create bar geometry, or place block ids
- material policy may consume these masks later, but material selection remains outside hydrology

## Invariants

1. masks must stay normalized to `0..1` where applicable
2. modes should describe the final hydrology state, not raw atlas wetness alone
3. material and voxel stages should not need to rediscover these masks from terrain geometry
