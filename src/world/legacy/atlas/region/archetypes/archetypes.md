# archetypes

## Role

- hold one submodule and one design note per concrete `RegionArchetype`
- keep regional identity, ecology notes, seasonal behavior, and meso allowances attached to the type itself

## Contract

- every concrete archetype has:
  - `mod.rs`
  - `<type>.md`
- the Rust module exposes a lightweight `RegionArchetypeDef`
- archetypes that need chunk-generation nuance may also expose optional runtime hints such as `PrototypeArchetypeHint` without moving the shared prototype solve into atlas
- the markdown file holds planning content:
  - stage label
  - biome family
  - terrain-form family
  - regional identity summary
  - ecology / seasonal planning notes
  - launch-primary meso direction
  - later-allowed meso pool, if any
  - material / seasonal policy hook once locked
- archetypes whose base landform is their primary identity may intentionally keep launch meso empty until prototype solving becomes authoritative

## Current Candidate Pool

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
