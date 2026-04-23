# time

## Role

- define fixed-tick time, calendar, season, climate-drift, and local-weather progression rules

## Responsibilities

- advance world calendar state on fixed tick boundaries
- handle minute / hour / day rollover deterministically
- update runtime atlas-cell temperature and humidity drift for active regions
- derive deterministic or seeded-probabilistic local weather outcomes from calendar plus climate state
- derive seasonal/ecology progression such as bloom progress, leaf-color shift, snow accumulation, thaw, or bare-branch transition
- decide whether effects become immediate nearby `WorldEdit`s or deferred seasonal patches for distant regions

## Non-Responsibilities

- storing the source-of-truth calendar state permanently
- raw input interpretation
- chunk loading policy
- renderer environment presentation

## Inputs

- fixed simulation tick
- active simulation region from ECS
- world calendar snapshot
- atlas runtime climate state
- region/archetype/climate regime context from world

## Outputs

- calendar/climate advancement result
- local weather state updates
- direct `WorldEdit`s for nearby realized regions
- deferred seasonal patch records for far-away regions
- dirty chunk / remesh hints when visual surface state changes

## Concrete Types

- `TimeSimConfig`
- `TimeSimBundleInput`
- `TimeSimCellInput`
- `TimeSimInput`

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

## Related Modules

- `simulation.md`
- `../world/calendar.md`
- `../world/surface/seasonal.md`
- `../ecs/fixed.md`

## Current Implementation Notes

- the current step advances `WorldCalendar.absolute_tick` every fixed tick
- the current minute boundary updates per-atlas-cell climate drift and local weather windows
- the current season-change path emits deferred `SetSeasonalState(...)` patches rather than direct block edits
- the current implementation is deterministic and tested, but it intentionally keeps nearby `WorldEdit` emission for later slices
