# axes

## Role

- document the classification dimensions that feed region resolution
- separate continuous climate and geography dimensions from resolved classes

## Continuous Dimensions

- `temperature_mean`
- `moisture_balance`
- `macro_elevation`
- `relief_energy`
- `drainage_potential`
- `coast_exposure`
- `thermal_seasonality`
- `precipitation_seasonality`
- `snow_persistence`
- `freeze_thaw_tendency`

## Resolved Dimensions

- `TemperatureBand`
- `MoistureBand`
- `ElevationBand`
- `ReliefClass`
- `HydrologyContext`
- `CoastalContext`
- `ClimateRegime`

## Notes

- `ClimateRegime` is derived from long-pattern climate dimensions rather than treated as a primary raw field
- later seasonal biome state should consume these resolved dimensions without rewriting the owning region archetype
