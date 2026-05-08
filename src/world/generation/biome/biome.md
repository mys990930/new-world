# biome

## Role

`biome` owns graph-first biome context and classification types for the new generation pipeline.

This module is a lightweight policy layer. It reads graph/macro-derived climate, elevation, water,
coast, basin, and mountain context and resolves a stable biome class that later field, surface, and
voxel stages can consume.

## Responsibilities

- Define graph-first final cell biome context.
- Define graph-first biome classes without depending on legacy atlas region cells.
- Split ocean biome meaning into shallow ocean and deep ocean.
- Classify coast, lake, wetland, dry basin, and climate-driven land biomes from deterministic inputs.
- Resolve detailed coast, wetland, dry/arid, cold/alpine, tropical, and temperate biome variants
  directly from `GraphBiomeContext`.

## Non-Responsibilities

- Voronoi graph construction.
- Macro ownership or elevation resolve.
- Hydrology routing.
- Material selection.
- Voxel fill.
- Preview binary rendering.

## Invariants

1. Water/coast/wetland roles take priority over climate-only land classification.
2. Ocean classification must distinguish `ShallowOcean` and `DeepOcean`.
3. Classification depends only on explicit context values and is deterministic.
4. Legacy `RegionClassCell` or atlas archetype types are not part of this contract.

## Current Classification Policy

- All inputs are clamped by `GraphBiomeContext::clamped` before classification. `temperature`,
  `hydration`, `coastness`, `mountainness`, and `basinness` use `0.0..=1.0`; `continentality` uses
  `-1.0..=1.0`; `elevation` remains signed macro elevation.
- Effective temperature is:

```text
effective_temperature = clamp01(temperature - mountainness * 0.14 - max(elevation, 0.0) * 0.08)
```

- Water-role priority:
  - `DeepOcean` -> `DeepOcean`.
  - `ShallowOcean` -> `ShallowOcean`.
  - `Lake` -> `Lake`.
  - `Coast`, `Wetland`, and `DryBasin` use their own detailed policies below before climate-only
    land classification.

### Coast

`GraphBiomeWaterRole::Coast` resolves in this order:

1. `RockyCoast` when `mountainness >= 0.58` or `elevation >= 0.42`.
2. `Mangrove` when `effective_temperature >= 0.68`, `hydration >= 0.72`, and `basinness >= 0.48`.
3. `EstuarineCoast` when `hydration >= 0.68` and `basinness >= 0.58`.
4. `LagoonCoast` when `coastness >= 0.72`, `elevation <= 0.08`, and `basinness >= 0.40`.
5. `SandyCoast` otherwise.

### Wetland

`GraphBiomeWaterRole::Wetland` resolves in this order:

1. `FloodedForest` when `hydration >= 0.78`, `effective_temperature >= 0.38`, and
   `mountainness < 0.48`.
2. `Swamp` when either `effective_temperature >= 0.52 && hydration >= 0.62` or
   `basinness >= 0.58`.
3. `Marsh` otherwise.

### Dry/Arid

`GraphBiomeWaterRole::DryBasin` and dry land with `hydration < 0.38` use the same dry/arid policy:

1. `PolarBarrens` when `effective_temperature <= 0.20`.
2. `Desert` when `hydration < 0.16` and `effective_temperature >= 0.52`.
3. `SemiDesert` when `hydration < 0.24`.
4. `Steppe` when `effective_temperature <= 0.46`.
5. `MediterraneanShrubland` when `0.46 <= effective_temperature <= 0.72`, `hydration >= 0.28`,
   and either `coastness >= 0.12` or `basinness < 0.55`.
6. `DryShrubland` when `hydration < 0.34`.
7. `Savanna` when `effective_temperature >= 0.68`.
8. `TemperateGrassland` otherwise.

### Cold/Alpine

- High mountain context is `elevation >= 0.68 && mountainness >= 0.56`. It resolves before ordinary
  cold, dry, tropical, or temperate land:
  1. `PolarIce` when `elevation >= 0.78` and `effective_temperature <= 0.12`.
  2. `PolarBarrens` when `effective_temperature <= 0.18` and `hydration < 0.26`.
  3. `Tundra` when `effective_temperature <= 0.18`.
  4. `SubalpineWoodland` when `effective_temperature <= 0.42` and `hydration >= 0.52`.
  5. `AlpineMeadow` otherwise.
- Non-alpine cold land resolves after high mountain context:
  1. `PolarIce` when `effective_temperature <= 0.08`.
  2. `PolarBarrens` when `effective_temperature <= 0.16` and `hydration < 0.28`.
  3. `Tundra` when `effective_temperature <= 0.28`.
  4. `BorealForest` when `effective_temperature <= 0.40` and `hydration >= 0.42`.
  5. `Steppe` when `effective_temperature <= 0.40` and `hydration < 0.42`.

### Tropical

Land with `effective_temperature >= 0.68` resolves after dry/arid checks:

1. `TropicalRainforest` when `hydration >= 0.76`.
2. `MonsoonForest` when `hydration >= 0.58`.
3. `TropicalDryForest` when `hydration >= 0.42`.
4. `Savanna` otherwise.

### Temperate

Remaining land resolves by hydration:

1. `TemperateRainforest` when `hydration >= 0.82`.
2. `TemperateMixedForest` when `hydration >= 0.58`.
3. `TemperateBroadleafForest` when `hydration >= 0.42`.
4. `TemperateGrassland` otherwise.

Distribution tests in `macro_map` verify that graph-first macro maps still expose ocean biomes,
grassland, dry/arid, tropical, and high-elevation alpine-meadow cells without changing water-role
priority.
