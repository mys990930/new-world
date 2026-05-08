# biome

## Role

`biome` owns graph-first final cell biome context and classification for the new generation
pipeline.

This module is a lightweight policy layer. It reads graph/macro-derived climate, elevation,
continentality, coast exposure, mountain context, ruggedness, and water role, then resolves a
stable biome class that later field, surface, and voxel stages can consume.

## Responsibilities

- Define graph-first final cell biome context.
- Define graph-first biome classes without depending on legacy atlas region cells.
- Split ocean biome meaning into shallow ocean and deep ocean.
- Classify ocean, coast, lake, wetland, dry basin, and climate-driven land biomes from
  deterministic explicit inputs.
- Use `continentality` to separate inland continental biomes from coastal or oceanic contexts.
- Use `ruggedness` to separate rocky coast, shrubland, and alpine/mountain cases from smooth
  plains and beaches.

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
4. `GraphBiomeContext` does not carry `basinness`; closed basin semantics are represented by
   `GraphBiomeWaterRole::DryBasin` before biome classification.
5. Legacy `RegionClassCell` or atlas archetype types are not part of this contract.

## Current Classification Policy

All inputs are clamped by `GraphBiomeContext::clamped` before classification. `temperature`,
`hydration`, `coastness`, `mountainness`, and `ruggedness` use `0.0..=1.0`; `continentality` uses
`-1.0..=1.0`; `elevation` remains signed macro elevation.

Effective temperature is:

```text
effective_temperature =
    clamp01(temperature - mountainness * 0.14 - ruggedness * 0.04 - max(elevation, 0.0) * 0.08)
```

The mountain/rugged/elevation cooling term makes high, rough terrain colder without changing the
source climate field. It is intentionally modest so that biome identity is still driven primarily
by graph temperature and hydration.

Water-role priority:

- `DeepOcean` -> `DeepOcean`.
- `ShallowOcean` -> `ShallowOcean`.
- `Lake` -> `Lake`.
- `Coast`, `Wetland`, and `DryBasin` use their own detailed policies before climate-only land
  classification.

### Coast

Coast is a shoreline transition. Common coast should mostly be sandy, with rocky coast as the
common rugged alternative. Mangrove, estuary, and lagoon are special cases, not defaults.

`GraphBiomeWaterRole::Coast` resolves in this order:

1. `RockyCoast` when `ruggedness >= 0.50`, `mountainness >= 0.58`, or `elevation >= 0.42`.
   This marks exposed rock, cliff, or mountain shore where waves meet resistant terrain.
2. `Mangrove` when `effective_temperature >= 0.68`, `hydration >= 0.76`, `coastness >= 0.68`,
   `ruggedness < 0.25`, and `continentality <= 0.35`.
   Mangroves need hot, wet, low-energy tropical shorelines, not steep or highly inland cells.
3. `EstuarineCoast` when `hydration >= 0.72`, `coastness >= 0.62`, `ruggedness < 0.32`, and
   `continentality <= 0.45`.
   Estuaries are wet, low-relief river-mouth or tide-influenced coast cells.
4. `LagoonCoast` when `0.56 <= coastness <= 0.82`, `elevation <= 0.06`, `hydration >= 0.66`,
   `ruggedness <= 0.12`, `mountainness <= 0.22`, and `0.05 <= continentality <= 0.42`.
   Lagoons are deliberately rare: flat, very low, wet, nearly smooth, and slightly landward
   pockets rather than the immediate exposed shoreline.
5. `SandyCoast` otherwise.
   Smooth non-special coasts become the dominant ordinary beach/strand category.

### Wetland

Wetlands are standing or saturated land roles supplied by macro/hydrology context.

1. `FloodedForest` when `hydration >= 0.78`, `effective_temperature >= 0.38`,
   `mountainness < 0.48`, `ruggedness < 0.36`, and `continentality >= -0.20`.
   This means warm-to-mild, wet, low-relief forested floodplain rather than open marsh.
2. `Swamp` when `effective_temperature >= 0.52`, `hydration >= 0.62`, and `ruggedness < 0.40`.
   Swamps are warm, wet, smooth wetlands.
3. `Marsh` otherwise.
   Marsh covers cooler, more open, or less forest-suitable wetlands.

### Dry/Arid

`GraphBiomeWaterRole::DryBasin` and dry land with `hydration < 0.38` use this dry/arid policy.
Inland dry biomes require or benefit from positive `continentality` and low `coastness` so oceanic
or shoreline cells do not dominate them.

1. `PolarBarrens` when `effective_temperature <= 0.20`.
2. `Desert` when `hydration < 0.16`, `effective_temperature >= 0.52`,
   `continentality >= 0.28`, and `coastness <= 0.45`.
   Hot deserts are strongly dry and inland/continental.
3. `SemiDesert` when `hydration < 0.24` and either `continentality >= 0.20` or
   `coastness <= 0.35`.
   Semi-desert is less extreme but still avoids broad wet/coastal influence.
4. `DryShrubland` when `ruggedness >= 0.42`, `hydration < 0.36`, and
   `continentality >= 0.12`.
   Dry shrubs prefer rougher slopes and broken terrain where grassland is less continuous.
5. `Steppe` when `effective_temperature <= 0.58` and `continentality >= 0.18`.
   Steppe is the cooler continental dry-grassland branch.
6. `MediterraneanShrubland` when `0.46 <= effective_temperature <= 0.72`,
   `0.28 <= hydration <= 0.46`, `coastness >= 0.18`, `continentality <= 0.45`, and
   `ruggedness < 0.45`.
   This captures mild, seasonally dry coastal or near-coastal shrubland rather than deep inland
   desert.
7. `Savanna` when the savanna condition below is met.
8. `TemperateGrassland` otherwise.

Savanna condition:

```text
effective_temperature >= 0.68
0.32 <= hydration <= 0.50
continentality >= 0.30
coastness <= 0.35
ruggedness < 0.58
```

Savanna is hot, seasonally dry, open, and mostly inland. Coastal tropical dry cells fall back to
`TropicalDryForest` instead of becoming broad shoreline savanna.

### Cold/Alpine

High alpine context is:

```text
elevation >= 0.68 && mountainness >= 0.56 && ruggedness >= 0.42
```

This prevents smooth high plateaus from becoming alpine just because they are high; alpine terrain
needs mountain context and rough relief.

High alpine resolves before ordinary cold, dry, tropical, or temperate land:

1. `PolarIce` when `elevation >= 0.78` and `effective_temperature <= 0.12`.
2. `PolarBarrens`/`Tundra` when `effective_temperature <= 0.18`, or when
   `ruggedness >= 0.72`, `effective_temperature <= 0.34`, and `hydration < 0.42`.
   The dry/rugged branch represents wind-scoured high barrens.
3. `SubalpineWoodland` when `effective_temperature <= 0.42` and `hydration >= 0.52`.
4. `AlpineMeadow` otherwise.

Non-alpine cold land resolves after high mountain context:

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
4. `Savanna` only when the inland savanna condition is met.
5. `TropicalDryForest` otherwise.

### Temperate

Remaining land resolves by hydration:

1. `TemperateRainforest` when `hydration >= 0.82`.
2. `TemperateMixedForest` when `hydration >= 0.58`.
3. `TemperateBroadleafForest` when `hydration >= 0.42`.
4. `TemperateGrassland` otherwise.

Distribution tests in `macro_map` verify that graph-first macro maps still expose ocean biomes,
grassland, dry/arid, tropical, and high-elevation alpine-meadow cells without changing water-role
priority.
