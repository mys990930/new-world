## simulation

### Role

- fixed-tick simulation rule execution core
- advance time-based gameplay and environment rules from world snapshots into deterministic results
- compute results without owning world storage directly

### Responsibilities

- execute fixed-tick subsystem steps
- define subsystem ordering and deterministic step boundaries
- consume world snapshot/query input and produce `SimulationResult`
- advance time/calendar/season/weather progression through world-owned contracts
- define chunk-scoped weather scalar progression through `weather.md`
- evaluate ecology, power, fluid, fire, farming, and later environment rules
- emit `WorldEdit`, `SimEvent`, dirty-chunk hints, and follow-up requests
- support region-scoped stepping so only active areas need eager simulation
- emit structured observer events for textmode diagnostics without embedding console strings in rule code
- use world-owned surface condition contracts when reporting wetness, snow cover, thaw, or frozen state

### Non-Responsibilities

- world source-of-truth ownership
- raw input handling
- gameplay command interpretation
- text formatting or console output
- owning the fixed-timestep accumulator itself
- worker-thread orchestration itself
- draw calls or GPU upload
- platform event handling

### Owned Data

#### Config / Runtime Data

- `SimulationConfig`
- `FixedStepConfig`
- `SimTick`
- `SubSystemId`
- `WeatherSimConfig`

#### Input Data

- `SimRegion`
- `SimInput`
- `SimInputBundle`
- `WeatherSimBundleInput`
- `WeatherSimChunkInput`

#### Output Data

- `SimulationResult`
- `SimEvent`
- `SimFollowupRequest`
- `SimulationResult::chunk_weather_updates` carries world-owned `ChunkWeatherUpdate` contracts
- ecology observer payloads are chunk-scoped structured events keyed by graph-first biome, not stored animal entities
- surface observer payloads are aliases/references to `world::SurfaceCondition` rather than simulation-owned storage
- weather observer payloads should be derived from world-owned chunk weather scalar state, not renderer-only values

### Use Cases

- ecology step
  - compute growth, spread, and natural-state changes
  - emit deterministic chunk-scoped ecology events such as biome-specific animal spawn candidates, animal conflicts, carcass creation, grazing, and plant growth stage changes
- power step
  - compute power graph / signal propagation in active chunks
- fluid step
  - compute water and other fluid spread
- fire step
  - compute burn / spread outcomes
- farming step
  - compute crop growth and state changes
- time/weather/season step
  - advance calendar/date/season progression
  - adjust active runtime climate context
  - update active chunk weather scalar state on in-game hour boundaries according to `weather.md`
  - derive deterministic local weather outcomes such as clear, cloudy, rain, snow, or storm from chunk scalars
  - emit chunk/region-scoped weather and surface-condition events that can be displayed by textmode or consumed later by renderer/gameplay systems
  - expose read-only local climate interpretation helpers so ECS/HUD can turn the same simulation signals into readable Celsius / humidity status
  - derive seasonal progression such as bloom, leaf-color change, snow accumulation, thaw, or bare-branch conversion
  - emit nearby `WorldEdit`s or far-away deferred seasonal patches through world-owned contracts
- textmode observer step
  - provide structured `SimEvent` data for `new-world-textmode` summaries
  - keep biome, weather, surface, ecology, and world-update meaning separate from final text formatting

### Interface

```rust
SimulationCore::new(config: SimulationConfig) -> SimulationCore
SimulationCore::config(&self) -> &SimulationConfig

SimulationCore::step(
    subsystem: SubsystemId,
    input: SimInput,
) -> SimulationResult

SimulationCore::step_all(
    tick: SimTick,
    region: SimRegion,
    input: SimInputBundle,
) -> Vec<SimulationResult>
```

Or per subsystem:

```rust
EcologySim::step(input: SimInput) -> SimulationResult
PowerSim::step(input: SimInput) -> SimulationResult
FluidSim::step(input: SimInput) -> SimulationResult
FireSim::step(input: SimInput) -> SimulationResult
FarmingSim::step(input: SimInput) -> SimulationResult
TimeSim::step(input: SimInput) -> SimulationResult
WeatherSim::step(input: SimInput) -> SimulationResult
```

### Dependencies

- `world`
- block registry / biome config / sim config

NOT:

- `platform`
- `renderer`
- concrete `app` orchestration
- concrete `ecs` or `jobs` implementation

### Invariants

1. simulation does not own world source-of-truth storage
2. simulation does not mutate world directly; it returns structured results
3. the same input must produce the same deterministic result
4. fixed-tick rules remain separate from render frame rate
5. subsystem execution order must be explicit
6. simulation results must identify affected chunks precisely
7. simulation rules do not read raw input or platform state directly
8. heavy work may be delegated to jobs, but the simulation rule meaning and result shape stay simulation-owned

### Submodules

- `time.md`: calendar, season, climate-drift, and weather progression rules
- `weather.md`: chunk-scoped weather scalar state, biome ranges, seasonal coefficients, thresholds, and renderer contract
- `ecology.md`: deterministic chunk-scoped ecology observer/candidate events
- `ecology_table.md`: biome-specific vegetation and animal candidate pools

### Notes

- world owns the source of truth for calendar, season phase, climate runtime state, and deferred environmental patches
- simulation owns how those values advance on fixed tick boundaries
- ECS should choose which regions are active enough for eager simulation and should consume the resulting state for gameplay and rendering bridges
- `new-world-textmode` is an observer/adapter over simulation results and world/ECS state; it must format structured data but must not become a new owner of simulation meaning

### Current Implementation Notes

- the first concrete implementation wires the `time`, `ecology`, and chunk `weather` subsystems
- `SimulationResult` can now carry a world-owned `CalendarAdvance` contract, chunk weather updates, plus generic `WorldEdit` / event / follow-up fields
- `SimulationResult::chunk_weather_updates` carries `ChunkWeatherUpdate` values; simulation computes them, while world applies and stores them
- `SimSurfaceCondition` is aligned to the world-owned `SurfaceCondition` contract so textmode output can later be replaced by renderer/gameplay consumers without changing rule meaning
- the same `time` module now also exports read-only local climate interpretation helpers so ECS HUD can display biome-consistent Celsius / humidity values without inventing a separate app-only climate scale
- chunk-scoped weather is implemented in `weather.rs` using graph biome/context input, previous state, neighbor state, seasonal coefficients, biome clamps, and hourly gating from `WeatherSimConfig::ticks_per_game_hour`
- old atlas local weather remains a migration compatibility path for existing renderer/app environment consumers
- ecology now has a deterministic observer slice that emits graph-biome-scoped animal/plant events for active chunks, without storing final entities
- power, fluid, fire, and farming remain planned subsystem boundaries but are not implemented yet
- the textmode observer slice should extend `SimEvent` first, then let app/binary-level code format one-second summaries from those structured events
