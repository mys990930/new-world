# environment

## Role

- derive a player-local environment snapshot from world-owned environmental truth
- keep the gameplay-side boundary for HUD, audio, and future local ambience consumers

## Responsibilities

- map the local player translation to the current atlas cell
- sample cached `RegionClassSample`, `WorldCalendar`, runtime climate state, and local weather from `WorldCore`
- convert simulation-owned climate signals into HUD-friendly Celsius and percent values
- expose one read-only local environment snapshot for app bridge consumption

## Non-Responsibilities

- owning the authoritative calendar, weather, or region-class data
- renderer sprite layout
- minimap cache ownership
- fixed-tick advancement rules

## Owned Data

### LocalEnvironmentStatus

- optional current `LocalEnvironmentSnapshot`
- last frame-local player-centered environment read model

### LocalEnvironmentSnapshot

- `atlas_coord`
- resolved `RegionClassSample`
- copied `WorldCalendar`
- copied `LocalWeatherState`
- simulation-owned `LocalClimateState`
- HUD-facing `LocalClimateDisplay`

## Inputs

- local player transform
- `WorldCore`
- simulation-owned local climate interpretation helpers

## Outputs

- one optional local environment snapshot that app bridge code may read without re-querying world state

## Public Interface

```rust
EcsRuntime::update_local_environment_from_world(world: &WorldCore)
EcsRuntime::local_environment_status() -> Option<LocalEnvironmentSnapshot>
```

## Invariants

1. ECS does not become the source of truth for calendar, climate, or weather
2. the same world snapshot and player atlas cell must produce the same local environment snapshot
3. biome and terrain labels shown in HUD come from `BiomeFamily / TerrainFormFamily`, not from app-owned ad-hoc strings
4. Celsius and humidity display values come from the same simulation-owned climate interpretation used for local weather derivation, with small weather-facing presentation clamps so snow/rain HUD readings stay believable
5. environment refresh must not synchronously resolve uncached atlas region classification; missing cache uses the default region sample until app/job warmup publishes the real one

## HUD Interpretation Notes

- biome and terrain should be shown as the resolved `BiomeFamily / TerrainFormFamily` pair
- temperature uses the simulation local climate signal and is displayed as a readable calibrated Celsius value rather than as the raw normalized atlas scalar
- rough display bands are intentionally human-readable:
  - `Polar` reads as persistent sub-freezing climate, roughly like high-latitude tundra or polar barrens
  - `Cold` reads around freezing to cool single digits for much of the year, closer to subarctic / boreal margins
  - `Temperate` reads as mild double-digit Celsius conditions, similar to many mid-latitude grassland or forest climates
  - `Warm` reads as low-to-mid 20s Celsius, similar to subtropical or Mediterranean warm seasons
  - `Hot` reads as high 20s to upper 30s Celsius, matching tropical lowlands or desert heat
- humidity percent is likewise interpreted from the local climate signal, then lifted by active overcast/rain/snow/storm presentation floors so the HUD does not show obviously dry air during precipitation

## Related Modules

- `ecs.md`
- `runtime.md`
- `../simulation/time.md`
- `../app/bridge_ui.md`
