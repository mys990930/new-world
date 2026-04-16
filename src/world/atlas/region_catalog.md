# region_catalog

## Role

- hold the planning draft for region-facing biome, terrain-form, archetype, and meso catalogs
- keep feature taxonomy visible in one place before further V2 implementation work
- act as the working design sheet that `region.md` and `meso.md` can later tighten into contracts

## Status

- this file is a planning draft, not an implementation contract
- names, splits, and allowed combinations may still change before V2 becomes authoritative
- the goal is to finish taxonomy and ownership first, then implement against that plan

## Design Goals

- separate climate or cover identity from terrain-shape identity
- resolve a stable `RegionArchetype` before meso feature selection
- keep large-scale direction in atlas skeleton and hydrology, not in per-chunk noise
- let meso add several-chunk readability without replacing the primary regional identity
- keep material borders one-hot by region ownership, with noisy boundaries rather than blended scalar thresholds
- keep the full planning catalog visible even if implementation still phases features in over time

## Layer Vocabulary

- `BiomeFamily`
  - climate, cover, and broad ecological feel
- `TerrainFormFamily`
  - dominant landform family
- `RegionArchetype`
  - the combined local identity used by base heightfield, meso allowance, and material policy
- `MesoFeature`
  - a several-chunk terrain accent applied inside an already-classified region archetype
- `SeasonalBiomeState`
  - the current in-year expression of that archetype under the current climate regime and calendar phase

## Raw Classification Dimensions

This section uses `dimensions` broadly on purpose. Some entries are raw continuous fields, while others are resolved contexts derived from those fields plus skeleton context.

The intended dimensions for archetype choice are:

- `TemperatureBand`
- `MoistureBand`
- `ElevationBand`
- `ReliefClass`
- `HydrologyContext`
- `CoastalContext`
- `ClimateRegime`

These should come from atlas raw fields plus skeleton context, then resolve deterministically into region classes before meso selection.

### Planned Continuous Climate Inputs

- temperature mean
- moisture balance
- continentality / coast exposure
- macro elevation
- relief energy / ruggedness
- drainage potential
- thermal seasonality
- precipitation seasonality
- snow-persistence tendency
- freeze-thaw tendency

### Planned Derived Regional Contexts

- `ReliefClass`
- `HydrologyContext`
- `CoastalContext`
- `ClimateRegime`

## Climate Regime And Seasonality Direction

- `ClimateRegime` should be derived from continuous climate dimensions, not authored as a disconnected label
- climate regime is meant to answer questions such as:
  - how strong are the seasons
  - how oceanic versus continental is the annual cycle
  - whether rainfall is evenly spread, monsoonal, or persistently arid
  - whether snow cover is brief, persistent, or nearly absent
- region archetype should capture the stable annual identity of a place
- seasonal biome state should capture the current in-year expression of that place

### Seasonal Biome State Direction

The later runtime seasonal layer should derive from:

- `RegionArchetype`
- `ClimateRegime`
- current day-of-year or season phase
- elevation
- local hydrology

That seasonal state can then drive surface and ecology transitions without replacing the underlying regional identity.

Examples:

- `temperate_plain`
  - spring/summer: grass-dominant surface
  - autumn: drier or browner grass variant
  - winter under snowy regimes: snowy-grass surface policy
- `cold_wet_lowland`
  - warm season: wet grass and exposed mud margins
  - cold season: frozen mud, partial ice, or snow-covered wet ground
- `tropical_seasonal_plain`
  - wet season: greener cover and fuller channels
  - dry season: duller cover and more exposed sediment
- `glaciated_alpine`
  - short thaw: exposed rock, patchy snow retreat
  - long cold season: persistent snow and stronger ice cover

## Draft Biome Families

### Aquatic / Coastal

- `open_ocean`
- `shelf_sea`
- `tidal_coast`
- `beach_coast`
- `lagoon_coast`
- `delta_coast`
- `cold_coast`

### Temperate / Mild

- `temperate_grassland`
- `temperate_forest`
- `temperate_wetland`
- `temperate_shrubland`

### Warm / Tropical

- `tropical_rainforest`
- `tropical_seasonal_forest`
- `savanna`
- `monsoon_wetland`

### Dry

- `steppe`
- `semi_arid_scrub`
- `desert`
- `salt_flat`

### Cold

- `boreal_forest`
- `cold_wetland`
- `tundra`
- `polar_desert`
- `snowfield`

### Alpine / Volcanic

- `alpine`
- `glaciated_alpine`
- `volcanic_barren`

## Draft Terrain Form Families

- `marine`
- `coast`
- `plain`
- `rolling_plain`
- `plateau`
- `hill`
- `ridge_upland`
- `mountain`
- `basin`
- `valley`
- `canyon`
- `wet_lowland`
- `dune`
- `badlands`
- `volcanic`
- `glacial`

## Draft Region Archetypes

Region archetypes combine biome family and terrain-form family into something generation can actually solve.

### Coastal / Water-Adjacent

- `shelf_shallows`
- `tidal_coast`
- `beach_plain`
- `coastal_cliffland`
- `lagoon_lowland`
- `delta_lowland`
- `cold_rocky_coast`

### Temperate

- `temperate_plain`
- `temperate_rolling_plain`
- `temperate_hills`
- `temperate_plateau`
- `temperate_basin`
- `temperate_wet_lowland`
- `temperate_ridge_upland`

### Warm / Tropical

- `tropical_rainforest_lowland`
- `tropical_rainforest_hills`
- `tropical_seasonal_plain`
- `savanna_plain`
- `savanna_hills`
- `monsoon_basin`
- `monsoon_delta`

### Dry

- `steppe_plain`
- `steppe_hills`
- `semi_arid_basin`
- `desert_plain`
- `desert_dune_sea`
- `desert_badlands`
- `salt_flat_basin`

### Cold / Alpine

- `boreal_plain`
- `boreal_hills`
- `tundra_plain`
- `cold_wet_lowland`
- `alpine_upland`
- `glaciated_alpine`
- `polar_basin`

### Volcanic / Special

- `volcanic_upland`
- `volcanic_plain`
- `caldera_basin`

## Example Deterministic Resolution Rules

- `temperate + subhumid + low elevation + low relief + inland`
  - `BiomeFamily = temperate_grassland`
  - `TerrainFormFamily = plain`
  - `RegionArchetype = temperate_plain`
- `temperate + subhumid + high elevation + low relief + inland`
  - `BiomeFamily = temperate_grassland`
  - `TerrainFormFamily = plateau`
  - `RegionArchetype = temperate_plateau`
- `hot + wet + low elevation + low relief`
  - `BiomeFamily = tropical_rainforest`
  - `TerrainFormFamily = plain`
  - `RegionArchetype = tropical_rainforest_lowland`
- `hot + wet + low-to-mid elevation + medium relief`
  - `BiomeFamily = tropical_rainforest`
  - `TerrainFormFamily = hill`
  - `RegionArchetype = tropical_rainforest_hills`
- `cold + wet + high elevation + high relief`
  - `BiomeFamily = glaciated_alpine`
  - `TerrainFormFamily = glacial`
  - `RegionArchetype = glaciated_alpine`

## Draft Meso Feature Catalog

### Core Upland Features

- `hill_cluster`
- `ridge_spur`
- `summit_group`
- `upland_terrace`
- `escarpment_band`

### Core Lowland Features

- `shallow_basin`
- `wet_basin`
- `broad_valley`
- `ravine`
- `sinkhole_field`

### Dry / Erosional Features

- `dune_field`
- `yardang_band`
- `badlands_patch`
- `alluvial_fan`

### Coastal Features

- `coastal_cliff_band`
- `cove_breakup`
- `barrier_spit`
- `lagoon_rim`

### Cold / Volcanic Features

- `glacial_trough`
- `crevasse_belt`
- `lava_field`
- `crater`

## First Implementation Candidate Set

Planning should keep the full candidate pool from the start, even if code implementation still phases actual support over time.

The first implementation wave should still prefer features that:

- read clearly inside a `2..6 chunk` play view
- do not need a separate 3D feature system
- do not depend on a fully rebuilt coast ecology stack
- can be expressed as deterministic base-heightfield deformation plus later hydrology and material policy

### Wave 1 Core

- `hill_cluster`
- `shallow_basin`
- `escarpment_band`
- `upland_terrace`

### Wave 1 Extension

- `ravine`
- `coastal_cliff_band`
- `dune_field`
- `crater`

## Draft Archetype -> Allowed Meso Matrix

This matrix is intentionally one-way. Meso may only choose from the allowed set under the current archetype.

### Temperate Plain Family

- `temperate_plain`
  - `hill_cluster`
  - `shallow_basin`
  - `broad_valley`
- `temperate_rolling_plain`
  - `hill_cluster`
  - `shallow_basin`
  - `ridge_spur`
- `temperate_hills`
  - `hill_cluster`
  - `ridge_spur`
  - `ravine`
- `temperate_plateau`
  - `escarpment_band`
  - `upland_terrace`
  - `shallow_basin`
- `temperate_basin`
  - `wet_basin`
  - `broad_valley`
  - `upland_terrace`
- `temperate_wet_lowland`
  - `wet_basin`
  - `broad_valley`
- `temperate_ridge_upland`
  - `ridge_spur`
  - `escarpment_band`
  - `ravine`

### Tropical / Warm Family

- `tropical_rainforest_lowland`
  - `broad_valley`
  - `wet_basin`
  - `hill_cluster`
- `tropical_rainforest_hills`
  - `hill_cluster`
  - `ridge_spur`
  - `ravine`
- `tropical_seasonal_plain`
  - `hill_cluster`
  - `broad_valley`
- `savanna_plain`
  - `hill_cluster`
  - `shallow_basin`
  - `alluvial_fan`
- `savanna_hills`
  - `hill_cluster`
  - `ridge_spur`
  - `ravine`
- `monsoon_basin`
  - `wet_basin`
  - `broad_valley`
  - `alluvial_fan`
- `monsoon_delta`
  - `broad_valley`
  - `barrier_spit`
  - `lagoon_rim`

### Dry Family

- `steppe_plain`
  - `hill_cluster`
  - `shallow_basin`
  - `alluvial_fan`
- `steppe_hills`
  - `hill_cluster`
  - `ridge_spur`
  - `ravine`
- `semi_arid_basin`
  - `shallow_basin`
  - `alluvial_fan`
  - `badlands_patch`
- `desert_plain`
  - `dune_field`
  - `alluvial_fan`
- `desert_dune_sea`
  - `dune_field`
  - `yardang_band`
- `desert_badlands`
  - `badlands_patch`
  - `yardang_band`
  - `ravine`
- `salt_flat_basin`
  - `shallow_basin`
  - `alluvial_fan`

### Coastal Family

- `beach_plain`
  - `barrier_spit`
  - `lagoon_rim`
- `coastal_cliffland`
  - `coastal_cliff_band`
  - `cove_breakup`
  - `upland_terrace`
- `lagoon_lowland`
  - `lagoon_rim`
  - `wet_basin`
- `delta_lowland`
  - `barrier_spit`
  - `broad_valley`
  - `alluvial_fan`
- `cold_rocky_coast`
  - `coastal_cliff_band`
  - `cove_breakup`

### Cold / Alpine Family

- `boreal_plain`
  - `hill_cluster`
  - `wet_basin`
- `boreal_hills`
  - `hill_cluster`
  - `ridge_spur`
  - `ravine`
- `tundra_plain`
  - `shallow_basin`
  - `wet_basin`
- `cold_wet_lowland`
  - `wet_basin`
  - `broad_valley`
- `alpine_upland`
  - `ridge_spur`
  - `escarpment_band`
  - `ravine`
- `glaciated_alpine`
  - `glacial_trough`
  - `crevasse_belt`
  - `ridge_spur`
- `polar_basin`
  - `shallow_basin`
  - `wet_basin`

### Volcanic Family

- `volcanic_upland`
  - `lava_field`
  - `crater`
  - `escarpment_band`
- `volcanic_plain`
  - `lava_field`
  - `crater`
- `caldera_basin`
  - `crater`
  - `wet_basin`

## Draft Material Policy Direction

- archetype should choose the default cover and subsoil policy
- hydrology should then override channel, lake, wetland, and floodplain materials
- coast context should override shoreline sediment and tidal materials
- seasonal biome state should override time-varying surface cover such as snowy grass, freeze-thaw mud, or wet-season versus dry-season appearance
- meso may bias local distribution, but it should not replace archetype ownership

Examples:

- `temperate_plain`
  - top cover: grass
  - shallow subsoil: dirt
  - rock exposure: stone on stronger slopes
- `desert_plain`
  - top cover: sand
  - shallow subsoil: compact sand / sandstone-like policy later
  - wet channels: mud or gravel only where hydrology says so
- `tropical_rainforest_lowland`
  - top cover: dense grass or litter-like soil policy later
  - shallow subsoil: rich dirt
  - wet pockets: mud under hydrology override
- `coastal_cliffland`
  - top cover: thin grass or sparse soil depending on moisture
  - exposed faces: stone
  - shoreline: beach sediment only where coast policy owns it

## Open Questions

- should `BiomeFamily` remain fairly broad while `RegionArchetype` carries most playable identity, or should biome families be more granular
- do we want `plateau` and `ridge_upland` as distinct terrain-form families at V2 launch
- should `wet_lowland` and `delta` live as terrain-form families, hydrology contexts, or only archetypes
- how explicit should the first runtime seasonal biome-state layer be: four broad seasons, continuous year fraction, or regime-specific wet/dry plus warm/cold phases
- how many coastal archetypes are worth carrying before coast-specific meso and material systems are rebuilt
- which archetypes need a dedicated first-pass block palette beyond grass, dirt, sand, mud, gravel, stone, snow, and water

## Immediate Next Planning Step

Before more V2 implementation, we should lock:

1. the final `BiomeFamily` list
2. the final `TerrainFormFamily` list
3. the minimum `RegionArchetype` launch set
4. the first meso matrix we are willing to support in code
5. the first seasonal biome-state model we are willing to support in code
