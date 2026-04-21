# meso_apply

## Role

- own the generation-side stage that resolves atlas-owned meso accents into feature-owned target surfaces on top of the base prototype
- keep cross-feature gating, compositing, and remaining-relief accounting centralized while concrete landform shape logic stays feature-owned

## Current / Target Types

- `MesoAppliedColumn`
- `MesoAppliedPrototype`
- feature-owned runtime helpers may also use private surface-sample structs such as:
  - `target_surface_y`
  - `blend_weight`
  - `relief_spend`
  - optional feature-local shape masks for later smoothing / hydrology protection

## Current Interface

```rust
build_chunk_meso_applied_prototype(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationV2Inputs,
    corridor_window: &ChunkCorridorWindow,
    prototype: &BaseHeightfieldPrototype,
) -> MesoAppliedPrototype
```

## Notes

- meso should refine the archetype-owned prototype, not replace it
- meso should not be treated as a generic per-column `delta_y` library once feature-owned landform resolvers exist
- each runtime-wired meso feature should own its own chunk-side resolve helper in the matching `atlas/meso/features/<feature>/` folder
- a feature-owned helper should read the prototype baseline plus atlas-owned guide context and return a target local surface with an explicit blend/falloff contract
- `meso_apply.rs` should stay responsible for:
  - archetype allowance gating
  - corridor attenuation / keep-out policy
  - feature ordering and compositing
  - shared relief-budget accounting
  - emitting the final per-column meso-applied surface for later stages
- the current launch implementation applies only the Wave 1 core features:
  - `hill_cluster`
  - `shallow_basin`
  - `escarpment_band`
  - `upland_terrace`
- chunk-side application currently uses `RegionArchetypeDef.allowed_meso_keys` as a temporary runtime gate until the authoritative per-archetype allowance matrix is fully locked
- avoid-primary-corridor features are attenuated near carried river corridors so meso does not close prototype outlets or overwrite the broad hydrology read
- the stage spends only part of each column's prototype relief budget and forwards the remaining budget to later smoothing and hydrology work
- `hill_cluster` is the first feature being moved from `generic delta_y` treatment to `feature-owned target-surface resolve`
  - sampled guide fields still provide broad cluster envelope and macro hill-strength context
  - the feature-owned runtime helper should turn those guides plus the base prototype into several broad hill lobes with explicit shoulders and a smoothly blended target surface
  - the resolved hill mass should rise from the plain through feature-owned falloff rather than by abruptly stacking a narrow additive delta on top of the base
- the other Wave 1 launch features may temporarily remain on legacy delta operators during the transition, but the target architecture is feature-owned surface resolution for every meso landform
