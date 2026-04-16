# sampler

## Role

- assemble the padded atlas windows that V2 generation scaffolding needs for a target chunk

## Responsibilities

- map `ChunkCoord` to the padded atlas footprint used by generation
- build the matching atlas raw-field window
- build the matching atlas skeleton window
- build the matching region-classification window
- build the matching meso-guide window

## Non-Responsibilities

- per-column bilerp sampling
- terrain-profile resolution
- hydrology carving
- material selection

## Notes

- after V1 removal, this module now exists only as a shared V2 input-assembly helper
- it should stay small and only gather chunk-aligned atlas context
