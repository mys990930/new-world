# corridors

## Role

- own river corridor and downstream-grade solving between region classification and biome-aware base prototype
- project atlas drainage structure plus region context into chunk-local water-corridor constraints
- give prototype a deterministic river/valley envelope before any meso deformation or final hydrology pass

## Responsibilities

- select the drainage branches and outlet context that matter for a target chunk footprint
- convert atlas-scale river-path segments into chunk-local corridor constraints
- expose broad channel-center, valley-seat, floodplain-width, and downstream-grade guidance
- preserve trunk / tributary / headwater / outlet continuity across chunk boundaries
- keep corridor solving deterministic and independent from chunk generation order
- keep basin outlets, coastal outlets, and upland headwaters visible as prototype constraints instead of late carve masks

## Non-Responsibilities

- final carved channel voxel shape
- final connected water-surface solve
- seasonal wetness/material overrides
- meso feature selection or application
- replacing the owning `RegionArchetype`

## Inputs

- `ChunkCoord`
- `ChunkGenerationV2Inputs`
- `AtlasStructureMap`
- `RegionClassMap`
- selected `AtlasFieldMap` hydrology and elevation tendencies when structure-only data needs bounded scalar support

## Outputs

- `ChunkCorridorWindow`

## Planned Interface

```rust
build_chunk_corridor_window(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationV2Inputs,
) -> ChunkCorridorWindow
```

The exact function name may still change, but the stage boundary should remain:

1. `inputs` owns atlas raw fields, skeleton, and region classification.
2. `corridors` turns those into branch-local water constraints.
3. `prototype` consumes those constraints before meso.

## Current Types

- `RiverCorridorConstraint`
- `ChunkCorridorWindow`

## Current Type Semantics

### `RiverCorridorConstraint`

- `center_x`, `center_z`
  - chunk-relative block-space center for the corridor influence sample
  - values may sit outside the strict `0..CHUNK_EDGE` footprint so neighboring branches can influence edge columns without seam artifacts
- `half_width_blocks`
  - broad prototype-scale corridor half-width, not final carved voxel-bank width
  - intended to bias valley seat, floodplain seat, and lowland opening before smoothing
- `downstream_grade_per_block`
  - macro downstream falloff used by prototype to keep longitudinal slope coherent
  - not the final water-surface quantization and not a promise about the eventual hydrology carve depth

### `ChunkCorridorWindow`

- owns every corridor constraint that can materially influence the target chunk or its immediate prototype margin
- should prefer a padded influence window over a strict in-chunk crop so prototype solve remains continuous near chunk borders

## Solve Inputs By Ownership

### From `structure`

- nearest river-path segments
- branch role such as headwater, tributary, trunk, or outlet reach
- confluence neighborhood
- downstream progress
- divide / pass / basin outlet context

### From `region`

- `HydrologyContext`
- `CoastalContext`
- `ElevationBand`
- `ReliefClass`
- `TerrainFormFamily`
- `RegionArchetype`

### From raw fields

- macro elevation tendency
- basinness / lake potential
- river-flow tendency
- wetness / aridity only as bounded support signals, not as a replacement for structure or region ownership

## Processing Direction

1. gather the padded structure and region windows already assembled for the target chunk
2. select the drainage branches whose corridor influence overlaps the target chunk or its prototype margin
3. classify each relevant reach as headwater, tributary, trunk, basin outlet, inland floodplain reach, or coastal outlet reach
4. derive a broad corridor centerline and longitudinal downstream grade from the selected path segments
5. derive corridor half-width from branch role, downstream progress, and bounded region context
6. emit `RiverCorridorConstraint` samples that prototype can consume without re-reading atlas structure directly

## Region Interaction Contract

- corridors come after region classification and may use region output as a constraint filter
- strong `HydrologyContext` from region classification is allowed to promote wetland, floodplain, basin, delta, or outlet-style corridor treatment during launch
- corridors must not rewrite the primary `RegionArchetype`; they only constrain where water-shaped prototype structure can appear inside or across those regions
- if region and structure disagree, structure owns long-range branch continuity while region owns the surrounding terrain identity and allowable corridor expression

## Prototype Integration Contract

- prototype should treat corridor constraints as pre-meso terrain-shape constraints
- the corridor stage is expected to define:
  - where the broad valley seat belongs
  - which direction downstream lowering must follow
  - where basin outlets or coastal exits must stay open
- prototype must not need to rediscover branch continuity from raw atlas fields once a `ChunkCorridorWindow` exists
- corridor constraints should remain broad and smooth enough that later smoothing can refine them without erasing the intended drainage network

## Invariants

1. the same `(seed, generator_version, chunk)` must always produce the same `ChunkCorridorWindow`
2. branch continuity must not depend on chunk generation order
3. corridor constraints must be strong enough to keep headwaters near upland structure and outlets near valid basin or coast exits
4. corridor solving must not silently degrade into isolated wet pockets that ignore drainage graph continuity
5. corridor width and downstream-grade guidance should stay smooth across neighboring chunks even when the owning branch crosses chunk borders

## Notes

- final corridor solve is still unimplemented in code
- this document now locks the stage boundary and solve semantics so implementation can follow without inventing ownership on the fly
