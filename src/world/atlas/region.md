# region

## Role

- resolve deterministic region classes from atlas raw fields plus skeleton context before chunk-local heightfield solving
- classify stable biome and terrain-form archetypes without collapsing everything into smooth scalar thresholds
- give generation a region-owned base identity that meso may later modulate, but not replace

## Responsibilities

- define the deterministic region-classification ownership layer that sits after atlas raw fields and skeleton
- resolve continuous atlas inputs into stable class bands rather than overlapping fuzzy weight bundles
- define biome-family, terrain-form-family, and combined region-archetype contracts
- expose sampleable region classification for generation, meso selection, and material policy
- keep classification deterministic, generation-order-independent, and on-demand

## Non-Responsibilities

- final river-channel carve or water voxel fill
- final chunk-local heightfield solve
- meso feature generation
- final block placement
- preview-only debug biome palettes

## Owned Data

- `RegionClassMap`
- `RegionClassCell`
- `RegionClassSample`
- `TemperatureBand`
- `MoistureBand`
- `ElevationBand`
- `ReliefClass`
- `HydrologyContext`
- `CoastalContext`
- `ClimateRegime`
- `BiomeFamily`
- `TerrainFormFamily`
- `RegionArchetype`

## Public Interface

```rust
resolve_region_classes(
    meta: &WorldMeta,
    area: AtlasArea,
    fields: &AtlasFieldMap,
    structure: &AtlasStructureMap,
) -> RegionClassMap

sample_region_classes(
    classes: &RegionClassMap,
    world_x: i32,
    world_z: i32,
) -> RegionClassSample
```

## Input Model

- continuous raw fields stay in `atlas_fields.md`
- current target raw axes:
  - temperature
  - moisture balance
  - macro elevation
  - macro relief energy / ruggedness
  - drainage potential
  - coast exposure / continentality
  - climate-regime tendency
- structure adds:
  - ridge proximity
  - drainage-path proximity
  - downstream progress
  - basin / divide context

## Resolved Class Model

- region classification should prefer stable resolved bands over ambiguous overlapping weight packs
- the target resolved class stack is:
  - `TemperatureBand`
  - `MoistureBand`
  - `ElevationBand`
  - `ReliefClass`
  - `HydrologyContext`
  - `CoastalContext`
  - `ClimateRegime`
- those resolved bands then combine into:
  - `BiomeFamily`
  - `TerrainFormFamily`
  - `RegionArchetype`

## Example Resolution Rules

- `temperate + subhumid + plain + low elevation` -> `temperate plain`
- `temperate + subhumid + plain + high elevation` -> `temperate plateau`
- `hot + wet + plain` -> `tropical rainforest lowland`
- `hot + wet + hill` -> `tropical rainforest hills`
- `cold + subhumid + mountain + strong relief` -> `cold mountain upland`

## Determinism Contract

- classification should not be "pick the biggest overlapping weight"
- classification should resolve to a stable archetype from:
  - normalized continuous fields
  - deterministic class-band thresholds
  - deterministic precedence rules
  - optional bounded tie-break rules keyed from `seed + region`
- if randomness is used, it must only break plausible ties inside a bounded candidate set

## Relationship To Meso

- region classification happens before meso
- meso must consume `RegionArchetype` as a constraint
- meso may choose between allowed local expressions inside a region, but it should not redefine the primary biome or terrain-form identity
- examples:
  - `temperate plain` may allow hill clusters or shallow basins
  - `temperate plateau` may allow escarpments or terraces
  - `tropical rainforest hills` may allow ravines or ridge spurs

## Relationship To Water

- drainage skeleton and river corridors should be defined before solving the biome-aware base heightfield
- region classification may use hydrology context, but it should not wait for final voxel water fill
- base heightfield solving should treat river corridors and basin outlets as constraints, not as late afterthought carve masks

## Invariants

1. the same `(seed, generator_version, area)` must always produce the same region classification
2. region classes must remain more stable than meso guides and less local than chunk micro detail
3. region classification must not depend on whole-world precomputation
4. meso should refine region identity, not replace it
5. final block materials should be derived from region classification plus local hydrology/material policy, not from raw scalar thresholds alone

## Current Status

- this layer is design-authoritative but not yet implemented in code
- current generation still resolves profile families directly from atlas-derived samples
- future work should move primary biome and terrain-form ownership into this layer before further meso expansion
