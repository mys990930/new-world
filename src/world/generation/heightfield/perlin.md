# heightfield/perlin

## Role

`perlin` owns optional small-scale heightfield relief for graph-first heightfield columns.

It is a child of `heightfield`, not a macro terrain owner. It adds optional small deterministic
block offsets inside the heightfield surface resolve. The default config is disabled, so existing
heightfield generation and previews keep `micro_relief_blocks = 0`. River bed, bank, and valley
variation are not owned here; they must be baked into the macro field before heightfield consumes
the column.

## Contract

- Determinism comes from `seed`, `generator_version`, and world-space `x/z`.
- It uses a small std-only 2D gradient Perlin fBM implementation.
- Preview-enabled defaults use about `8` blocks amplitude and clamp output to `10` blocks.
- Preview-enabled relief is applied before contour-band resolve so the band source is less visibly
  stair-stepped.
- Lake columns and genuinely submerged ocean source columns receive `0` land micro relief.
- Ocean-owned source terrain at sea level and in a shallow below-sea border band uses the same land
  micro relief map as ordinary land, so the last sea-level border layer does not become an overly
  clean line. The band is block-scale and derived from the configured Perlin max displacement
  depth.
- Ocean-owned source terrain above sea level also uses the ordinary land micro relief map.
- Ocean columns may receive separate bounded bed relief when enabled. This offset applies only to
  the terrain bed, fades in away from the immediate shoreline, and never moves the sea-level water
  surface. Sea-level or above-sea ocean-owned terrain receives no bed-relief water movement from
  this pass, and ocean bed relief must not create a water column.
- River core columns may receive a small bounded bed relief when Perlin is enabled. This relief is
  applied before contour-band resolve, uses the same deterministic world-space Perlin map family as
  land, and is capped below ordinary land relief so it reads as noisy riverbed contour descent rather
  than a second river carve. Broad river bank/shoulder morphology and Q-driven depth still belong to
  `macro_field`; this pass only perturbs the already resolved river core source before snapping.
- Land, ridge, and dry basin columns may receive relief.

## Non-Goals

- Macro ownership changes.
- Macro bathymetry or river surface solving.
- New crates or chunk-local random state.
