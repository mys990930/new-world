# sampler

## Role

- Bridge from chunk-space generation to atlas-space control fields.

## Responsibilities

- map `ChunkCoord` to the atlas neighborhood required for generation
- sample four surrounding atlas cells
- bilerp atlas scalar fields per block column
- expose enough hydrology/climate signal for generation to choose bed and surface materials, not just relief

## Notes

- Atlas cells are macro control points, not final block outcomes.
- The sampler is intentionally separate from profile resolution so generation can evolve its own interpretation of atlas data without rewriting chunk-to-atlas lookup.
- Distance-driven atlas fields such as coast and river proximity may depend on context outside the four bilerp corners, so generation must preserve the field semantics instead of recomputing them from a tiny local mask.
- Generation currently asks atlas for a padded neighborhood around the target chunk so coast, continent-core, and hydrology signals do not collapse when a chunk sits inside a tiny 2x2 local slice.
- The next revision should use that same padded neighborhood to gather mountain-spine and drainage-path structure, not just scalar corner values, so chunk-local realization keeps macro direction across chunk boundaries.
