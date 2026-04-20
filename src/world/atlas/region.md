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
- expose archetype-owned runtime hint data when later generation stages need concrete per-archetype tuning without giving up shared solver ownership
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
region_archetype_prototype_hint(
    id: RegionArchetype,
) -> Option<&'static PrototypeArchetypeHint>
```

## Input Model

- continuous raw fields stay in `atlas_fields.md`
- current target raw classification dimensions:
  - temperature mean
  - moisture balance
  - macro elevation
  - macro relief energy / ruggedness
  - drainage potential
  - coast exposure / continentality
  - thermal seasonality
  - precipitation seasonality
  - snow-persistence tendency
  - freeze-thaw tendency
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
- `ClimateRegime` is a derived long-pattern class built from climate dimensions plus regional context; it is not intended to be an unrelated standalone raw field
- those resolved bands then combine into:
  - `BiomeFamily`
  - `TerrainFormFamily`
  - `RegionArchetype`
- the authoritative launch candidate lists and launch policy direction for `BiomeFamily`, `TerrainFormFamily`, and `RegionArchetype` now live in `region_catalog.md`
- `region/catalog.md` mirrors the same launch `RegionArchetype` pool as the implementation-facing index

## Seasonality And Climate Regime

- `ClimateRegime` should be derived from long-pattern climate dimensions such as thermal seasonality, precipitation seasonality, continentality, and snow-persistence tendency
- region classification should use that derived regime to choose a stable annual identity, not to decide the exact current-month visual state
- `RegionArchetype` should stay relatively stable across the year
- a later seasonal biome-state layer should derive from:
  - `RegionArchetype`
  - `ClimateRegime`
  - current world calendar position
  - elevation and local hydrology modifiers
- examples:
  - a `temperate_plain` may stay the same archetype all year while surface cover shifts between green grass, autumn-dry grass, and snowy grass
  - a `cold_wet_lowland` may keep the same archetype while water margins freeze seasonally and thaw later
  - a `tropical_seasonal_plain` may keep the same archetype while wet-season versus dry-season cover and saturation change

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
- launch policy allows `HydrologyContext` to override otherwise generic inland biome or terrain-form resolution when wetland, alluvial, or lake-basin ownership is strong enough to claim a dedicated archetype
- that override should happen during region resolution, not later as a surface-only material exception
- base heightfield solving should treat river corridors and basin outlets as constraints, not as late afterthought carve masks

## Invariants

1. the same `(seed, generator_version, area)` must always produce the same region classification
2. region classes must remain more stable than meso guides and less local than chunk micro detail
3. region classification must not depend on whole-world precomputation
4. meso should refine region identity, not replace it
5. final block materials should be derived from region classification plus local hydrology/material policy, not from raw scalar thresholds alone

## Current Status

- an initial deterministic atlas-cell classification scaffold is now implemented in code
- the current scaffold resolves banded classes, biome families, terrain-form families, and coarse region archetypes from atlas raw fields plus nearby skeleton presence
- raw classifier logic may now resolve the broader scaffolded archetype pool first, while the public `resolve_region_classes(...)` path still applies launch fallback at the final step for downstream safety
- the biome-family and terrain-form-family candidate taxonomies are now locked at the planning level
- the launch candidate pool in `region_catalog.md` is now the authoritative planning contract for classifier-facing regional identity
- a full `RegionArchetype` candidate pool with `launch`, `extended`, and `deferred` labels is now scaffolded as per-type modules and docs
- archetype modules may now also attach optional generation-facing prototype hints so concrete launch archetypes can diverge inside the shared prototype solver without forking the whole algorithm
- launch policy now treats strong hydrology context as eligible to override generic inland archetype selection
- the next planning step is to lock archetype-to-meso allowance and surface policy from that candidate pool
- current generation still resolves final profile families directly from atlas-derived samples, so this region layer is not active gameplay authority yet
- future work should promote this layer into the primary owner of biome and terrain-form identity before further meso or material expansion

## Submodules

- `axes.md`
- `catalog.md`
- `archetypes/archetypes.md`
