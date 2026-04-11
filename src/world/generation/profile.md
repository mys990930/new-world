# profile

## Role

- Resolve the dominant generation-side terrain profile for a column from atlas weights.

## Current Profiles

- `DeepOcean`
- `Shelf`
- `Coast`
- `Plain`
- `Upland`
- `Ridge`

## Notes

- These profiles are generation-owned realization categories, not authoritative final biome ids.
- Atlas stays responsible for macro scalar fields and biome tendencies; generation decides which surface shaper to invoke for actual chunk terrain.
- The same `TerrainProfile` enum is surfaced through generation probe APIs so debug tooling can report which profile dominated a chunk or column, even though final surface height can now be blended from multiple neighboring profile shapers.
- Ridge promotion is intentionally more aggressive than preview-biome classification because terrain realization needs sharper transitions between rolling uplands and steep mountain spines.
