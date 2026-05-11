# time

## Role

- define fixed-tick time, calendar, season, and climate boundary progression rules
- provide hour/day boundaries used by chunk weather simulation

## Responsibilities

- advance world calendar state on fixed tick boundaries
- handle minute / hour / day rollover deterministically
- update runtime atlas-cell temperature and humidity drift for active regions
- provide calendar, season, and hour-boundary inputs for chunk-scoped weather rules in `weather.md`
- expose read-only helpers that interpret the same local climate signals into HUD-friendly Celsius and relative-humidity displays
- derive seasonal/ecology progression such as bloom progress, leaf-color shift, snow accumulation, thaw, or bare-branch transition
- decide whether effects become immediate nearby `WorldEdit`s or deferred seasonal patches for distant regions

## Non-Responsibilities

- storing the source-of-truth calendar state permanently
- raw input interpretation
- chunk loading policy
- renderer environment presentation
- owning chunk weather scalar policy

## Inputs

- fixed simulation tick
- active simulation region from ECS
- world calendar snapshot
- atlas runtime climate state
- region/archetype/climate regime context from world

## Outputs

- calendar/climate advancement result
- hour-boundary signals and climate context consumed by weather simulation
- direct `WorldEdit`s for nearby realized regions
- deferred seasonal patch records for far-away regions
- dirty chunk / remesh hints when visual surface state changes

## Concrete Types

- `TimeSimConfig`
- `TimeSimBundleInput`
- `TimeSimCellInput`
- `TimeSimInput`
- `LocalClimateState`
- `LocalClimateDisplay`

## Processing Scale

- every fixed tick
  - advance the simulation clock
- every minute boundary
  - update active atlas-cell temperature/humidity drift
  - re-evaluate local weather windows
- every hour boundary
  - advance slow seasonal progress values such as bloom or leaf-color interpolation
- every day boundary
  - advance date / day-of-year / season phase
  - refresh longer seasonal windows and deterministic event seeds

## Invariants

1. time and season simulation remains deterministic for the same world seed and calendar position
2. eager whole-world updates are not required; active simulation plus lazy catch-up is valid
3. distant environmental changes may be stored as deferred patches instead of immediate chunk mutation
4. weather and seasonal state should derive from world calendar plus climate context rather than from ad-hoc renderer-only logic
5. chunk weather scalar policy is specified in `weather.md`; time only supplies deterministic temporal inputs

## Related Modules

- `simulation.md`
- `weather.md`
- `../world/calendar.md`
- `../world/surface/seasonal.md`
- `../ecs/fixed.md`

## Current Implementation Notes

- the current step advances `WorldCalendar.absolute_tick` every fixed tick
- the current minute boundary updates per-atlas-cell climate drift and legacy local weather windows
- future chunk weather should update on in-game hour boundaries using `weather.md`
- the current season-change path emits deferred `SetSeasonalState(...)` patches rather than direct block edits
- the current implementation is deterministic and tested, but it intentionally keeps nearby `WorldEdit` emission for later slices
- HUD-facing temperature / humidity now deliberately reuse the same local climate signal that weather derivation reads:
  - the underlying simulation signal is still normalized and solver-oriented
  - ECS converts that signal into a readable Celsius / percent view through read-only helpers in this module
  - that display mapping is intentionally calibrated for human readability rather than treated as a literal raw-atlas scalar dump
- rough climate interpretation targets are:
  - `Polar`: persistent sub-freezing climate, comparable to high-latitude tundra or polar barrens
  - `Cold`: around freezing to cool single digits for much of the year, closer to subarctic / boreal margins
  - `Temperate`: mild double-digit Celsius climate, similar to many mid-latitude plains and forests
  - `Warm`: low-to-mid 20s Celsius, similar to subtropical or Mediterranean warm seasons
  - `Hot`: high 20s to upper 30s Celsius, comparable to equatorial lowlands or desert heat
- display humidity is likewise derived from the local climate signal, then clamped upward during overcast/rain/snow/storm presentation so HUD readouts stay believable next to the active weather state
