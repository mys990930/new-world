# chunk weather worker plan

## Goal

Implement chunk-scoped weather scalar simulation that can drive textmode first and renderer later.

Weather must be replaceable by real gameplay/rendering systems, so simulation emits structured state and world stores source-of-truth data. Console text is only an adapter.

## Required Weather State

- Per loaded/active chunk:
  - `temperature`
  - `moisture`
  - `cloud`
  - `rain`
  - derived weather kind: `clear`, `cloudy`, `rain`, `snow`, `storm`
- Values update every in-game hour.
- Value targets derive from:
  - biome-specific base ranges
  - current chunk graph/Voronoi temperature and hydration
  - neighboring chunk weather values
  - biome-specific seasonal coefficients
  - previous chunk weather values
- Weather kind changes when thresholds are crossed.
- Renderer later reads the same scalar state for lighting, color temperature, fog, wetness, and precipitation presentation.

## Step 1 - Specs

- Update official module docs before code.
- Add `src/simulation/weather.md`.
- Document biome weather profile ranges, seasonal coefficients, neighbor blending, hourly update, thresholds, and renderer contract.
- Update refs:
  - `src/simulation/simulation.md`
  - `src/simulation/time.md`
  - `src/world/world.md`
  - `src/renderer/renderer.md`
  - `temp-worker-docs/new-world-textmode-plan.md`
- References: `context.md`, `src/world/generation/biome/biome.md`, `src/simulation/ecology_table.md`, `src/simulation/time.rs`, `src/world/legacy/calendar.rs`.

## Step 2 - World Contract

- Add world-owned chunk weather state contract.
- Suggested public shape:
  - `ChunkWeatherState { temperature, moisture, cloud, rain, kind, updated_at_tick }`
  - `ChunkWeatherUpdate { coord, state }`
  - `WorldCore::chunk_weather(coord)`
  - `WorldCore::set_chunk_weather(coord, state)` or apply method
- Keep old atlas `LocalWeatherState` available as compatibility until renderer/app migration is done.
- References: `src/world/world.md`, `src/world/legacy/calendar.rs`, `src/world/legacy/core.rs`, `src/world/legacy/core.md`.

## Step 3 - Weather Simulation

- Add a weather subsystem or weather leaf under `simulation`.
- Inputs:
  - active chunks
  - `GraphBiomeKind`
  - graph/Voronoi temperature and hydration context
  - previous chunk weather
  - neighbor chunk weather
  - `WorldCalendar.season_phase`
- Output structured weather updates/events.
- Update every in-game hour, not every tick.
- Threshold draft:
  - `storm`: `cloud >= 0.82 && rain >= 0.72`
  - `snow`: `rain >= 0.35 && temperature <= 0.18`
  - `rain`: `rain >= 0.40`
  - `cloudy`: `cloud >= 0.38`
  - otherwise `clear`
- References: `src/simulation/mod.rs`, `src/simulation/time.rs`, `src/simulation/weather.md`, `src/world/generation/biome/mod.rs`.

## Step 4 - Textmode Adapter

- Make `new-world-textmode` display chunk weather scalar state.
- Per chunk weather line target:
  - `weather : Cloudy temp=0.62 moist=0.44 cloud=0.71 rain=0.18`
- Keep graph-first biome sampling.
- Keep console formatting in binary only.
- References: `src/bin/new-world-textmode.rs`, `src/bin/new-world-textmode.md`, `src/bin/bin.md`.

## Step 5 - Renderer Bridge

- Update renderer environment contract to consume chunk weather scalar state plus time of day.
- Presentation intent:
  - `cloud`: lower direct light, soften ambient, increase fog
  - `rain`: lower saturation/contrast, increase wetness and precipitation strength
  - `temperature`: shift color temperature
  - `moisture`: haze/fog/vegetation tint input
  - `storm`: darken light, cool tint, high fog/rain strength
- References: `src/renderer/renderer.md`, `src/app/fixed.rs`, `src/app/bridge_ui.rs`.

## Verification

- Run `cargo fmt`.
- Run focused tests for changed modules.
- Run `cargo check --bin new-world-textmode`.
- For textmode steps, run `cargo run --bin new-world-textmode -- --seconds 2`.

## Reporting

- Report changed docs/code and why.
- Report verification commands and results.
- Commit coherent completed stages only. Do not push.
