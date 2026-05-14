# heightfield/perlin

## Role

`perlin` owns the optional surface micro-relief pass for graph-first heightfield columns.

It is a child of `heightfield`, not a macro terrain owner. It adds small deterministic block
offsets inside the heightfield surface resolve. The default config is disabled, so existing
heightfield generation and previews keep `micro_relief_blocks = 0`.

## Contract

- Determinism comes from `seed`, `generator_version`, and world-space `x/z`.
- It uses a small std-only 2D gradient Perlin fBM implementation.
- Preview-enabled defaults use about `8` blocks amplitude and clamp output to `10` blocks.
- Preview-enabled relief is applied before contour-band resolve so the band source is less visibly
  stair-stepped.
- Ocean and lake columns always receive `0` micro relief.
- River columns currently receive `0` micro relief to protect river continuity.
- Land, ridge, and dry basin columns may receive relief.

## Non-Goals

- Macro ownership changes.
- Ocean/lake bathymetry or river surface solving.
- New crates or chunk-local random state.
