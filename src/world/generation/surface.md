# surface

## Role

- Build the chunk-local surface field that generation and probes share before block fill.

## Responsibilities

- sample per-column atlas inputs for the target chunk plus padding
- resolve the dominant terrain profile for each sampled column
- evaluate the raw profile-blended `surface_y`
- smooth that heightfield in world-space so neighboring columns read as continuous terrain
- derive a local concavity signal that hydrology can use for river carving

## Inputs

- `ChunkCoord`
- `WorldMeta`
- `AtlasFieldMap`

## Outputs

- `ChunkSurfaceField`
- `PreparedSurfaceColumn`

## Notes

- Smoothing happens before hydrology, so rivers carve into an already-continuous terrain surface instead of trying to correct high-frequency block jitter afterward.
- The field includes padding outside the target chunk so smoothing and local-concavity queries remain deterministic at chunk borders.
- This module does not place blocks; it only prepares the shared surface scaffold used by probes and realization.
