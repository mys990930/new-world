# seasonal

## Role

- define the runtime seasonal biome-state layer that sits above region archetype

## Responsibilities

- describe in-year surface states such as snowy temperate ground or tropical wet season
- let time-varying appearance change without reclassifying the owning archetype
- stay as world-owned seasonal-state definitions that simulation can advance and ECS can consume

## Current Stub States

- `TemperateGrowing`
- `TemperateSnowy`
- `TropicalWetSeason`
- `TropicalDrySeason`
- `ColdFrozenWetland`
- `AlpineSnowpack`
- `CoastalStormSeason`

## Notes

- this file defines the stable runtime seasonal-state vocabulary, not the fixed-tick progression loop
- world should own the authoritative seasonal state and any deferred far-region seasonal patches
- simulation should own how those states advance over time
- the current chunk-generation path may resolve one of these states only when an explicit runtime context is provided
- the baseline `generate_chunk(...)` path still keeps seasonal state optional so static generation does not invent world calendar ownership
