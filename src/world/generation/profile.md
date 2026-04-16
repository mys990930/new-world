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
- Atlas should own raw fields, skeleton, and future region classification; generation decides which surface shaper to invoke for actual chunk terrain after consuming that classification.
- The same `TerrainProfile` enum is surfaced through generation probe APIs so debug tooling can report which profile dominated a chunk or column, even though final surface height can now be blended from multiple neighboring profile shapers.
- Ridge promotion is intentionally more aggressive than preview-biome classification because terrain realization needs sharper transitions between rolling uplands and steep mountain spines.
- `Coast`, `Plain`, `Upland`, and `Ridge` should not be treated as the future meso feature set. They are broad shape families that future meso guides may bias or modulate.
- In the target V2 model, profile resolution should happen after region classification and river-corridor setup, then feed the biome-aware base heightfield solve before meso deformation is applied.
