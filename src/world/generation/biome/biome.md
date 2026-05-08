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

- Effective temperature is graph temperature reduced by mountainness and positive macro elevation.
- `Alpine` is not a generic cold mountain fallback. It requires high signed macro elevation plus
  mountain context; lower mountain cells can still classify as tundra, boreal forest, or another
  climate biome.
- Temperate dry land resolves to `TemperateGrassland` before it is treated as forest.
- The hot band uses wider hydration thresholds so `HotDesert`, `Savanna`,
  `TropicalSeasonalForest`, and `TropicalRainforest` remain reachable in bounded deterministic
  seed scans.
- Distribution tests in `macro_map` verify that grassland, hot dry/wet tropical classes, and
  high-elevation Alpine cells appear without changing water-role priority.
