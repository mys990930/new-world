# profiles

## Role

- Hold the profile-specific surface shaping functions used by generation.

## Responsibilities

- dispatch from `TerrainProfile` to the matching surface function
- keep ocean, coast, inland, and ridge shaping logic isolated
- expose blended profile surface evaluation so neighboring profiles can ease into each other instead of creating hard height seams
- keep profile-local relief broad enough to describe contour shape while leaving final smoothing to `surface.md`

## Current Modules

- `ocean.md`
- `coast.md`
- `plain.md`
- `upland.md`
- `ridge.md`

## Notes

- Each profile module turns the same sampled atlas signals into a different height curve and relief pattern.
- Generation can now blend several of those curves together near boundaries, while still reporting a dominant profile for debug and fill heuristics.
- This keeps future material, vegetation, and structure generation aligned around the same profile boundary.
- `upland` now owns broken highland relief, while `ridge` is expected to exaggerate crags, escarpments, and sharper vertical transitions instead of reading like a taller plain.
- These profile functions now intentionally bias toward lower-frequency shape; final chunk generation smooths the combined field before hydrology and block fill.
- Those profile-specific shapers are still local realization tools. A later meso layer is expected to sit above them and decide where multi-chunk hill clusters, cliff bands, basins, or similar readable terrain features should appear.
