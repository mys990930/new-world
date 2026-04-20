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
- `hill_cluster` now uses two signals together:
  - the sampled guide fields still provide the broad cluster envelope and overall inland hill strength
  - the feature-owned `hill_cluster` runtime helper synthesizes deterministic chunk-space hilllets from nearby strong guide cells so the final deformation reads as several local hills
- the apply operator keeps `hilliness` as an edge falloff and broad weight, but preserves more peak height so inland hill groups can read at the current `0.5m` block scale
