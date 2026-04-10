# sampler

## Role

- Bridge from chunk-space generation to atlas-space control fields.

## Responsibilities

- map `ChunkCoord` to the atlas neighborhood required for generation
- sample four surrounding atlas cells
- bilerp atlas scalar fields per block column

## Notes

- Atlas cells are macro control points, not final block outcomes.
- The sampler is intentionally separate from profile resolution so generation can evolve its own interpretation of atlas data without rewriting chunk-to-atlas lookup.
- Distance-driven atlas fields such as coast and river proximity may depend on context outside the four bilerp corners, so generation must preserve the field semantics instead of recomputing them from a tiny local mask.
