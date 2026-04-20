# meso_apply

## Role

- own the step that applies atlas-owned meso accents on top of the base prototype

## Current Types

- `MesoAppliedColumn`
- `MesoAppliedPrototype`

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
- the current launch implementation applies only the Wave 1 core features:
  - `hill_cluster`
  - `shallow_basin`
  - `escarpment_band`
  - `upland_terrace`
- chunk-side application currently uses `RegionArchetypeDef.allowed_meso_keys` as a temporary runtime gate until the authoritative per-archetype allowance matrix is fully locked
- avoid-primary-corridor features are attenuated near carried river corridors so meso does not close prototype outlets or overwrite the broad hydrology read
- the stage spends only part of each column's prototype relief budget and forwards the remaining budget to later smoothing and hydrology work
- `hill_cluster` now uses two layers together:
  - sampled guide fields still provide the broad cluster envelope and macro hill-strength context
  - the feature-owned `hill_cluster` runtime helper now resolves neighboring strong guide cells into coherent asymmetric macro-lobe chains during meso apply
- the launch tuning intentionally leans on feature-owned lobe resolution more than the broad guide field so `hill_cluster` reads as several nearby hills with higher local relief instead of as round guide blobs
- the apply operator keeps generic gating, relief-budget spend, and corridor attenuation in `meso_apply.rs`, while feature-specific hill-cluster lobe layout stays in `hill_cluster/mod.rs`
