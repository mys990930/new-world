# calendar

## Role

- define the world-owned calendar, seasonal progression state, runtime climate state, and deferred seasonal patch contract

## Responsibilities

- own the canonical world calendar state
- own date and season identifiers that other layers read but do not author
- own atlas-cell runtime climate state such as current local temperature and humidity drift
- own deferred seasonal/weather/ecology patch records for regions that are not currently realized in loaded chunks
- define lazy catch-up rules for regions that were not simulated eagerly while far away

## Owned Data

### WorldCalendar

- current absolute tick or time cursor
- minute, hour, day, and day-of-year
- season phase / season index

### AtlasClimateRuntimeState

- per-atlas-cell runtime temperature offset
- per-atlas-cell runtime humidity offset
- last-updated simulation time for lazy catch-up

### DeferredSeasonPatch

- atlas-cell or chunk-column target
- deterministic patch kind such as flower bloom, leaf tint shift, snow cover, freeze/thaw, or bare-branch conversion
- authored-at calendar position
- apply-on-realization / apply-on-interest-entry behavior

### LocalWeatherState

- deterministic local weather outcome such as clear, rain, snow, storm
- start/end window or current phase
- source atlas cell / climate context

## Inputs

- fixed-tick simulation results
- world calendar advancement requests
- realized world context needed when deferred patches are finally applied

## Outputs

- read-only calendar and seasonal state for ECS, simulation, jobs, and renderer bridges
- deferred patch queues for later realization
- lazy catch-up surfaces for active-region realization

## Public Interface

```rust
WorldCore::calendar(&self) -> &WorldCalendar
WorldCore::climate_state(coord: AtlasCoord) -> AtlasClimateRuntimeState
WorldCore::local_weather(coord: AtlasCoord) -> Option<LocalWeatherState>
WorldCore::deferred_season_patches(&self) -> &[DeferredSeasonPatch]
WorldCore::apply_calendar_advance(advance: CalendarAdvance) -> CalendarApplyResult
```

## Invariants

1. world owns the source of truth for calendar/date/season state
2. ECS may read world calendar state, but it does not author the calendar directly
3. simulation may advance calendar/climate state only through world-owned data contracts
4. far-away regions may be represented as deferred seasonal state rather than eager block mutation
5. the same seed plus the same calendar position must produce deterministic deferred seasonal outcomes

## Related Modules

- `world.md`
- `surface/seasonal.md`
- `../simulation/time.md`

## Notes

- this layer is meant to keep long-lived environmental truth in world even when only a small active region is simulated eagerly
- nearby chunks may receive direct `WorldEdit` application, while distant chunks may receive deferred patches that are realized later
- the current first slice stores runtime climate/weather state per atlas cell and appends deferred seasonal patches, but it does not yet mutate nearby realized blocks for seasonal visuals
- the current default bootstrap calendar now starts at `11:00` on spring day `0` so daytime lighting and HUD state begin from a neutral midday-ish slice instead of early evening
