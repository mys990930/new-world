# weather

## Role

- Define the world-owned chunk weather scalar storage contract used by the current runtime bridge.
- Keep chunk weather query/apply data structured while old atlas-cell `LocalWeatherState` consumers migrate.

## Responsibilities

- define `ChunkWeatherKind`
- define `ChunkWeatherState`
- define `ChunkWeatherUpdate`
- define `WeatherApplyResult`
- clamp weather scalar values at the world API boundary

## Non-Responsibilities

- hourly weather simulation rules
- weather kind threshold derivation
- renderer presentation
- text formatting
- replacing legacy atlas-cell `LocalWeatherState`

## Owned Data

### ChunkWeatherState

- `temperature: f32`
- `moisture: f32`
- `cloud: f32`
- `rain: f32`
- `kind: ChunkWeatherKind`
- `updated_at_tick: u64`

All scalar fields are normalized to `0.0..=1.0` when applied through `WorldCore`.

### ChunkWeatherKind

- `Clear`
- `Cloudy`
- `Rain`
- `Snow`
- `Storm`

### ChunkWeatherUpdate

- `coord: ChunkCoord`
- `state: ChunkWeatherState`

### WeatherApplyResult

- `coord`
- `previous`
- `current`
- `changed`

## Public Interface

```rust
ChunkWeatherState::clear(updated_at_tick: u64) -> ChunkWeatherState
ChunkWeatherState::clamped(self) -> ChunkWeatherState
WorldCore::chunk_weather(coord: ChunkCoord) -> Option<ChunkWeatherState>
WorldCore::set_chunk_weather(coord: ChunkCoord, state: ChunkWeatherState) -> WeatherApplyResult
WorldCore::apply_chunk_weather_update(update: ChunkWeatherUpdate) -> WeatherApplyResult
```

## Invariants

1. `world` owns the stored chunk weather state.
2. Simulation computes `ChunkWeatherUpdate`; it does not mutate storage directly.
3. Renderer, textmode, and gameplay consumers read the same chunk weather state.
4. Atlas-cell `LocalWeatherState` remains available until old consumers migrate.
5. Scalar values stored through the public apply path stay in `0.0..=1.0`.

## Related Modules

- `core.md`
- `calendar.md`
- `../world.md`
- `../../simulation/weather.md`
