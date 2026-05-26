# heightfield/perlin

`perlin.rs` owns optional small-scale deterministic micro relief for heightfield columns.

The default config is disabled. When enabled, Perlin relief is applied before or around contour-band
resolve according to `HeightfieldPerlinPlacement`.

Contract:

- Determinism comes from `seed`, `generator_version`, and world-space `x/z`.
- Lake columns and genuinely submerged ocean source columns receive no land micro relief.
- Ocean bed relief is a separate bounded terrain-bed perturbation and never moves the sea-level
  water surface.
- River hints do not receive a special Perlin path in the reset state. River-hinted land samples use
  the same ordinary land relief as any other land sample.
- Perlin must not change macro ownership, water ownership, or terrain kind.
