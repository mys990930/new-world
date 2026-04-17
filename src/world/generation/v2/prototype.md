# prototype

## Role

- own the biome-aware base heightfield prototype before meso deformation
- consume `ChunkCorridorWindow` as a pre-meso terrain constraint layer

## Responsibilities

- turn `RegionArchetype` plus corridor constraints into a broad terrain scaffold
- respect valley seats, floodplain openings, basin outlets, and coastal exits already decided by the corridor stage
- leave local accents and later hydrology detail to later stages instead of re-solving long-range branch logic here

## Inputs

- `ChunkCoord`
- `ChunkGenerationV2Inputs`
- `ChunkCorridorWindow`

## Outputs

- `BaseHeightfieldPrototype`

## Current Types

- `PrototypeColumn`
- `BaseHeightfieldPrototype`

## Notes

- this is intentionally a stub
- later implementation should turn region archetype and corridor constraints into broad terrain shape here
- prototype should treat corridor output as authoritative drainage-shape guidance rather than rediscovering rivers from raw atlas scalar fields
