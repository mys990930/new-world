# ravine

## Stage

- `launch`

## Identity

- placement family: `ValleyFloor`
- hydrology coupling: `AvoidPrimaryCorridor`

## Summary

- Feature-owned runtime resolver for deterministic medium-size ravine valleys.

## Terrain Traits

- Atlas/shared meso remains responsible for broad guide context only; the ravine folder owns runtime resolution into explicit world-space ravine objects.
- Runtime resolve must not re-roll a different trench per chunk or per sampled column.
- Resolved ravines are owned by a stable `MesoRegion` window and then sampled by neighboring chunks from that same resolved object set.
- Each resolved ravine keeps narrow trench depth and broader shoulder lowering as separate signals so transition fill cannot fake the deepest incision.
- Launch tuning should read as a narrow drainage valley with an upstream head, a stronger downstream incision, sharper trench-floor cuts, and uneven valley shoulders rather than as a generic basin or a short straight scratch.
- Resolved ravines should use a feature-owned medium meso footprint, usually several chunks long, and should not be constrained to one guide-cell footprint.
- Resolved ravine silhouettes should vary within bounded parameters, but their centerlines should use rough angular kinks rather than broad smooth S-curves.
- Width and depth should vary along the centerline: upstream heads start narrower and shallower, middle reaches hold the main trench, and downstream/outlet reaches widen before tapering into the surrounding surface.
- The runtime helper should return a surface-oriented sample with `target_surface_y`, `blend_weight`, and `relief_spend` rather than a raw shared `delta_y`.
- Current runtime resolve is intentionally provisional with respect to guide inputs: until dedicated ravine guide channels exist, it derives sparse high-confidence source candidates from existing basin/escarpment/terrace context in `MesoGuideCell`.
- Should avoid displacing major river corridors and instead sit beside or above them.

## Size Class

- class: `medium meso`
- target footprint: about `3..8` chunks long after source pruning
- source spacing: much wider than one guide cell, so adjacent guide peaks should merge into one resolved ravine reach instead of many short cuts
- owner padding: may read neighboring `MesoRegion` guide sources so a reach can cross chunk and meso-cell boundaries without changing shape
- density: very sparse; broad launch plains and wet lowlands should not inherit ravines just because their fallback archetype happens to allow generic lowland meso
- launch allowance: prefer rugged hill, ravine-country, alpine, escarpment, badlands, karst, or other erosion-prone archetypes; ordinary plain/lowland launch fallback targets should usually withhold `ravine`

## Runtime Contract

- `build_ravine_window(...)` builds a feature-owned resolved window for one chunk request.
- `sample_ravine_apply_signal_from_window(...)` exposes trench/shoulder debug and apply signals from that resolved window.
- `sample_ravine_surface_from_window(...)` returns the feature-owned target local surface used by later meso apply orchestration.
- Debug tooling may use `debug_ravine_candidates(...)` to inspect which meso cells are even entering ravine resolve before owner-region pruning.

## Ownership And Sampling

- ownership unit: `MesoRegion` with neighboring-owner padding
- resolved object space: world-space ravine centerline plus upstream/downstream width, depth, meander, and taper parameters
- sampling rule: neighboring chunks overlapping the same ravine footprint must observe the same resolved object and same world-space result
- seam rule: chunk or meso-cell boundaries may cross a ravine footprint, but they must not re-own or rotate the ravine there

## Ecology Notes

- Later ecology can use these cuts to channel denser vegetation, shade, or runoff-biased cover.
- Later material policy can also use trench versus shoulder masks to bias exposed rock, talus, wet sediment, or seasonal snow persistence differently.

## Preview / Test Targets

- deterministic resolved window for the same seed and guide map
- stable resolved ownership across neighboring chunk contexts
- no visible seam for the same world-space point at chunk boundaries
- flat response when relevant guide context is absent
- trench center deeper than shoulders, with shoulders broader than trench coverage
- downstream samples should generally resolve wider/deeper than upstream head samples before the terminal taper
- a resolved ravine should have a length-to-floor-width ratio high enough to read as a valley reach instead of a compact oval pit
- visibly lowered cut survives surface sampling with normal relief budgets
- centerline samples should contain angular local deflections rather than one smooth sinusoidal S-curve

## Follow-up

- add dedicated ravine guide channels to shared meso emission once main-thread integration work is ready
- hook `build_ravine_window(...)` and `sample_ravine_surface_from_window(...)` into `meso_apply.rs`
- tune corridor attenuation and cross-feature compositing once shared runtime wiring lands
