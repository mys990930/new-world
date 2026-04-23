# surface

## Role

- own material policy, cover overrides, seasonal biome-state vocabulary, and chunk-column surface-plan resolution
- sit between region archetype identity and final voxel block assignment

## Responsibilities

- default material policy ids and definitions
- seasonal biome-state ids and definitions
- cover override rules such as snowy grass or frozen mud
- resolve one chunk-column surface owner per sampled region archetype
- choose per-column top, filler, core, and water block keys from archetype policy plus hydrology
- quantize terrain / standing-water tops for voxelization without re-solving hydrology

## Non-Responsibilities

- atlas raw classification
- meso feature placement
- final hydrology geometry
- final block writes into `ChunkData`

## Current Submodules

- `material.md`
- `seasonal.md`
- `cover.md`
- `resolve.md`
