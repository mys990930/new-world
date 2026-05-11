# surface

## Role

- own material policy, cover overrides, seasonal biome-state vocabulary, runtime surface-condition observations, and chunk-column surface-plan resolution
- sit between region archetype identity and final voxel block assignment

## Responsibilities

- default material policy ids and definitions
- seasonal biome-state ids and definitions
- chunk/atlas surface-condition vocabulary for dry, wet, snow-covered, half-thawed snow, and frozen states
- normalized wetness, snow depth, and thaw values for textmode, renderer, and gameplay observers
- cover override rules such as snowy grass or frozen mud
- coherent material-domain selection between atlas region influence and final block stacks
- resolve one chunk-column surface owner per sampled region archetype
- choose per-column top, filler, core, and water block keys from archetype policy plus hydrology
- quantize terrain / standing-water tops for voxelization without re-solving hydrology or raising water above the hydrology-provided level

## Non-Responsibilities

- atlas raw classification
- meso feature placement
- final hydrology geometry
- final block writes into `ChunkData`
- console text formatting

## Current Submodules

- `material.md`
- `seasonal.md`
- `cover.md`
- `condition.md`
- `domain.md`
- `resolve.md`
