# hydrology

## Role

- own final hydrology solve after corridors and prototype shape already exist

## Responsibilities

- resolve connected waterlines, channel floors, floodable openings, and final water-surface tendencies
- consume canonical drainage ownership rather than discovering water independently per chunk
- preserve continuity with neighboring tiles through shared anchor-aware water and floor constraints

## Non-Responsibilities

- primary biome classification
- broad prototype landform selection
- final material or voxel block choice

## Continuity Contract

- hydrology should not start from independent per-chunk searches for local minima or channel candidates
- it should reuse canonical branch ownership plus the shared tile and border-anchor contract from `continuity.md`
- neighboring tiles should agree on water-surface tendency, outlet floor tendency, and protected channel direction before voxelization

## Planned Solve Strategy

1. gather canonical drainage ownership and downstream boundary tendencies for the shared continuity tile
2. consume the smoothed tile plus hydrology-facing anchor samples
3. solve connected channels, outlets, and floodable surfaces while keeping shared border tendencies aligned
4. crop the requested chunk's hydrology result from the shared solve tile

## Current Types

- `HydrologySolve`

## Notes

- connected waterlines belong here, not in raw region classification
