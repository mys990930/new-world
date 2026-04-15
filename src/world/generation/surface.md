# surface

## Role

- Build the chunk-local surface field that generation and probes share before block fill.

## Responsibilities

- sample per-column atlas inputs for the target chunk plus padding
- sample or rasterize nearby atlas structure guides for the same padded footprint
- sample nearby atlas-owned meso guides over that same footprint
- resolve the dominant terrain profile for each sampled column
- evaluate the raw profile-blended `surface_y`
- bias that raw scaffold with meso and ridge/channel guide weights before smoothing
- smooth that heightfield in world-space so neighboring columns read as continuous terrain
- derive a local concavity signal that hydrology can use for river carving

## Inputs

- `ChunkCoord`
- `WorldMeta`
- `AtlasFieldMap`
- `AtlasStructureMap`
- `MesoGuideMap`

## Outputs

- `ChunkSurfaceField`
- `PreparedSurfaceColumn`

## Notes

- Smoothing happens before hydrology, so rivers carve into an already-continuous terrain surface instead of trying to correct high-frequency block jitter afterward.
- The field includes padding outside the target chunk so smoothing and local-concavity queries remain deterministic at chunk borders.
- Nearby mountain spines and river paths are rasterized into per-column structure weights before smoothing, so chunk-local relief can already lean toward the atlas-owned macro skeleton.
- `along_channel_cells` now represents downstream progress along the owning river branch, not just distance inside a single projected segment.
- Explicit drainage `Confluence` nodes are also sampled into the guide so later hydrology can widen and deepen tributary joins.
- Wave 1A meso guides now bias broad hill, basin, escarpment, and terrace shape before final smoothing.
- Meso remains a guide layer rather than a hard feature label map; this module consumes blended channels and combines them with profile families plus structure.
- This module does not place blocks; it only prepares the shared surface scaffold used by probes and realization.
