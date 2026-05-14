# heightfield/perlin

## Role

`perlin` owns optional small-scale heightfield relief for graph-first heightfield columns.

It is a child of `heightfield`, not a macro terrain owner. It adds small deterministic block
offsets inside the heightfield surface resolve. The default config is disabled, so existing
heightfield generation and previews keep `micro_relief_blocks = 0` and river bed relief disabled.

## Contract

- Determinism comes from `seed`, `generator_version`, and world-space `x/z`.
- It uses a small std-only 2D gradient Perlin fBM implementation.
- Preview-enabled defaults use about `8` blocks amplitude and clamp output to `10` blocks.
- Preview-enabled relief is applied before contour-band resolve so the band source is less visibly
  stair-stepped.
- Ocean and lake columns always receive `0` micro relief.
- Ocean columns may receive separate bounded bed relief when enabled. This offset applies only to
  the terrain bed, fades in away from the immediate shoreline, and never moves the sea-level water
  surface.
- River columns keep `micro_relief_blocks = 0`, but preview-enabled config may add stronger
  bounded Perlin offsets to the river terrain bed and the adjacent river bank/shoulder field. River
  water surface height is calculated from the unperturbed bed so this pass does not own water
  continuity.
- Land, ridge, and dry basin columns may receive relief.

## Non-Goals

- Macro ownership changes.
- Macro bathymetry or river surface solving.
- New crates or chunk-local random state.
