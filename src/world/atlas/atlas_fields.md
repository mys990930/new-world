# atlas_fields

## Role

- compute continuous atlas-scale raw fields and derived macro-environment factors
- provide deterministic raw inputs for skeleton solving, region classification, and later terrain realization

## Responsibilities

- land / ocean mask
- continent id / continent core factor
- macro elevation / continentality / coast exposure
- macro ruggedness / relief energy / ridge / mountain mass
- basin / drainage / river / lake potential
- temperature / humidity / inlandness
- aridity / wetness / polar / alpine / ecotone factor
- climate-regime tendencies and other long-pattern signals used by later region classification
- expose stable continuous axes instead of trying to decide final biome ownership directly

## Non-Responsibilities

- final biome or terrain-form classification
- mountain/drainage topology ownership
- meso feature selection
- final block placement

## Processing Order

1. compute landness and continent structure
2. compute ocean/coast distance and continent metadata
3. compute macro elevation, ridge tendency, and mountain mass
4. compute drainage, river, and lake potentials
5. compute temperature, humidity, inlandness, aridity, and wetness
6. compute climate-regime tendencies and other derived raw axes for later region classification

## Revision Direction

- current code still computes ridge / mountain / river signals mostly as scalar atlas fields
- the next revision should insert a structural pass between continent setup and final regional interpretation
- that structural pass owns mountain-chain spines, passes, drainage routing, and river corridor continuity
- atlas scalar fields such as `ridge_factor`, `mountain_mass`, `river_distance_estimate`, and part of `riverine_factor` should gradually become projections derived from that skeleton instead of unrelated local noise only
- resolved biome and terrain-form classes should move out of overlapping fuzzy weights and into a separate region-classification layer
- raw fields should stay continuous, while deterministic banding and archetype resolution should happen in `region.md`

## Weight / Axis Direction

- prefer a smaller set of clearer raw continuous axes over many partially overlapping weight packs
- current target raw inputs for region classification are:
  - temperature
  - moisture balance
  - macro elevation
  - relief energy / ruggedness
  - drainage potential
  - coast exposure / continentality
  - climate regime tendency
- slope should usually be treated as a downstream heightfield-derived signal rather than as the primary atlas raw field

## Invariants

1. scalar fields must stay within documented normalized ranges
2. hydrology must remain coherent with land/ocean structure
3. distance-style fields must not collapse to zero just because a small sampled area lacks a local source cell
4. default tuning values live in `tuning.rs`, and field generation composes them rather than hard-coding alternate defaults elsewhere
5. reference seed scans used during tuning should still contain some non-trivial mountain signal so generation can realize more than rolling uplands
6. mountain and river scalar guidance should remain compatible with a future atlas-owned structure graph rather than baking chunk-local randomness into macro direction
7. raw field outputs should be suitable for deterministic region classification without requiring ad-hoc fuzzy overlap to decide final biome identity
