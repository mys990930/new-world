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

## Locked Biome Families

### Ocean / Coast

- `Oceanic`
- `RockyCoast`
- `SandyCoast`
- `EstuarineCoast`
- `LagoonCoast`
- `Mangrove`

### River / Lake

- `Marsh`
- `Swamp`
- `FloodedForest`

### Desert / Dryland

- `Desert`
- `SemiDesert`
- `Steppe`
- `DryShrubland`
- `MediterraneanShrubland`

### Temperate Plain / Forest

- `TemperateGrassland`
- `TemperateBroadleafForest`
- `TemperateMixedForest`
- `TemperateRainforest`
- `BorealForest`

### Tropical

- `Savanna`
- `TropicalDryForest`
- `TropicalRainforest`
- `MonsoonForest`

### Mountain

- `SubalpineWoodland`
- `AlpineMeadow`

### Cold

- `Tundra`
- `PolarBarrens`
- `PolarIce`

## Locked Terrain Form Families

### Ocean / Coast

- `MarineShelf`
- `BeachPlain`
- `BarrierCoast`
- `LagoonCoast`
- `RockyShore`
- `SeaCliff`
- `EstuaryLowland`
- `FjordCoast`
- `Delta`

### River / Lake

- `Floodplain`
- `WetLowland`
- `AlluvialLowland`

### Plain To Mountain

- `Plain`
- `RollingPlain`
- `HillCountry`
- `Pediment`
- `MountainFront`
- `HillCluster`
- `Mountain`
- `AlluvialFan`
- `Plateau`
- `DuneField`

### Escarpment / Fracture

- `MesaCountry`
- `Escarpment`
- `Badlands`
- `Karst`

### Basin / Valley / Canyon

- `Basin`
- `NarrowValley`
- `BroadValley`
- `GlacialValley`
- `Canyon`
- `RavineCountry`
- `RidgeCountry`

### Polar / Ice

- `Icefield`
- `CrevassedIcefield`

## Locked RegionArchetype Candidate Pool

Region archetypes combine one locked `BiomeFamily` with one locked `TerrainFormFamily` into a playable regional identity. The pool below is now scaffolded in per-type modules and docs.

### Launch

- `oceanic_shelf`
- `sandy_beach_plain`
- `coastal_cliffland`
- `cold_wet_lowland`
- `temperate_plain`
- `temperate_hills`
- `temperate_plateau`
- `steppe_plain`
- `desert_plain`
- `desert_dune_field`
- `savanna_plain`
- `tropical_rainforest_lowland`
- `tropical_rainforest_hills`
- `glaciated_alpine`
- `tundra_plain`

### Extended

- `rocky_shore_coast`
- `barrier_coast`
- `lagoon_coast`
- `estuary_lowland`
- `coastal_delta`
- `mangrove_lagoon`
- `mangrove_delta`
- `marsh_floodplain`
- `swamp_lowland`
- `flooded_forest_alluvial_lowland`
- `flooded_forest_floodplain`
- `temperate_rolling_plain`
- `temperate_basin`
- `temperate_broad_valley`
- `temperate_escarpment_upland`
- `temperate_broadleaf_plain`
- `temperate_mixed_hills`
- `boreal_plain`
- `boreal_hills`
- `boreal_wet_lowland`
- `steppe_hills`
- `semi_desert_pediment`
- `dry_shrubland_badlands`
- `dry_shrubland_karst`
- `mediterranean_shrubland_hills`
- `desert_basin`
- `desert_mesa_country`
- `savanna_hills`
- `tropical_dry_forest_hills`
- `monsoon_floodplain`
- `subalpine_wooded_front`
- `alpine_meadow_mountain`
- `polar_barrens_plain`
- `monsoon_delta`
- `crevassed_icefield`
- `glacial_valley`
- `desert_alluvial_fan`

### Deferred

- `fjord_coast`
- `boreal_ridge_country`
- `monsoon_plateau`
- `alpine_ravine_country`

## Example Deterministic Resolution Rules

- `temperate + subhumid + low elevation + low relief + inland`
  - `BiomeFamily = TemperateGrassland`
  - `TerrainFormFamily = Plain`
  - `RegionArchetype = temperate_plain`
- `temperate + subhumid + high elevation + low relief + inland`
  - `BiomeFamily = TemperateGrassland`
  - `TerrainFormFamily = Plateau`
  - `RegionArchetype = temperate_plateau`
- `hot + wet + low elevation + low relief`
  - `BiomeFamily = TropicalRainforest`
  - `TerrainFormFamily = Plain`
  - `RegionArchetype = tropical_rainforest_lowland`
- `hot + wet + low-to-mid elevation + medium relief`
  - `BiomeFamily = TropicalRainforest`
  - `TerrainFormFamily = HillCountry`
  - `RegionArchetype = tropical_rainforest_hills`
- `cold + wet + high elevation + high relief`
  - `BiomeFamily = PolarIce`
  - `TerrainFormFamily = Icefield`
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

## Archetype-To-Meso Status

- every scaffolded archetype now carries an initial meso allowance stub inside its per-type module and markdown file
- those stub allowances are intentionally provisional
- the next planning pass should lock:
  - which meso features remain legal per archetype
  - which launch archetypes need stricter or broader meso sets
  - which current stubs should be emptied until later waves

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

## Next Planning Step

With the archetype candidate pool now scaffolded, the next planning step is:

1. confirm or trim the `launch` archetype set
2. lock the authoritative archetype-to-meso allowance matrix
3. lock the first seasonal biome-state model for launch archetypes
4. lock the first material/block policy per launch archetype
5. only after that, start V2 implementation against those locked policies
