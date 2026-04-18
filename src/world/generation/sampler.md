# sampler

## Role

- assemble the padded atlas windows that V2 generation scaffolding needs for a target chunk

## Responsibilities

- map `ChunkCoord` to the padded atlas footprint used by generation
- keep neighboring chunks on a stable structure-sized atlas-context tile so overlapping inputs do not shift at every atlas-cell boundary
- assemble raw atlas scalar cells from canonical region-owned field patches so the same `AtlasCoord` does not drift when requested by different chunks
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
- the current implementation snaps chunk input assembly to a structure-sized `8 x 8` atlas-cell context tile before adding padding, so nearby chunks reuse the same broad atlas/structure neighborhood
- raw atlas scalar values are no longer taken directly from a per-chunk temporary solve window; each requested `AtlasCoord` is copied from a canonical region-owned field solve with generous halo padding so overlapping chunks see identical values for identical atlas cells
