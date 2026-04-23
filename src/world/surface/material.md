# material

## Role

- define stable material policies for region archetypes before seasonal overrides

## Current Launch-Oriented Policies

- `OceanicShelf`
- `SandyBeach`
- `SteppeGrassland`
- `SavannaGrassland`
- `TemperateGrassland`
- `TemperatePlateau`
- `TropicalLowland`
- `TropicalHills`
- `DesertSurface`
- `ColdWetland`
- `AlpineExposed`
- `TundraExposure`
- `CoastalCliff`

## Notes

- these policy ids now carry an initial runtime block palette for:
  - default top
  - dry top
  - wet top
  - frozen top
  - shallow filler
  - deep core
  - exposed rock
  - hydrology sediment fallback
- the palettes are still launch-oriented and intentionally simple
- exact per-archetype ownership still starts from `RegionArchetype`, then resolves through one policy id instead of letting material thresholds redefine the region
