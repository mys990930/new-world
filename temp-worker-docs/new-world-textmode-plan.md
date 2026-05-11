# new-world-textmode worker plan

## Goal

Implement a `new-world-textmode` binary that observes real simulation/world/ECS contracts through console text.

The text output is an adapter only. Do not put presentation strings inside `simulation`, `ecs`, or `world` core state.

## Required Output

- Every real second, refresh the console until `Ctrl+C` and show current `YY-MM-DD HH:MM (season)`.
- Load or realize a temporary `3x3` chunk area around the current chunk.
- Print each loaded chunk as a cell inside a visible `3x3` box-drawing grid each second.
- Per chunk line, include:
  - cell biome
  - current weather
  - surface state: wet, snow-covered, half-thawed snow, etc.
  - multiple ecology events: animal spawn, animal fight, carcass created, plant grazed, plant growth stage
  - world update records, if any update was applied or requested

## Design Constraints

- `simulation` emits structured events/results, never console text.
- `world` owns source-of-truth state and applies structured world updates.
- `ecs` selects active simulation scope and receives gameplay/entity-facing events.
- `new-world-textmode` formats structured state/events into text.
- Keep all contracts replaceable by future non-text gameplay/rendering systems.

## Steps

1. Update specs first.
   - Minimal context: textmode is an observer binary and must not own simulation meaning.
   - Touch: `src/simulation/simulation.md`, `src/ecs/ecs.md`, `src/ecs/fixed.md`, `src/world/world.md`, `src/app/fixed.md` if app fixed orchestration changes.
   - References: `context.md`, `AGENTS.md`, `src/simulation/time.md`, `src/app/fixed.md`.

2. Extend structured simulation events.
   - Minimal context: `SimulationResult.events` already exists; extend `SimEvent` rather than logging in rule code.
   - Candidate event groups: weather status, surface condition, ecology event, world update requested/applied.
   - References: `src/simulation/mod.rs`, `src/simulation/time.rs`, `src/simulation/simulation.md`.

3. Add first surface-condition contract.
   - Minimal context: surface wetness/snow/thaw state should be world-readable and renderer-replaceable later.
   - Start at chunk-level state unless finer scope is required by docs.
   - References: `src/world/world.md`, `src/world/legacy/calendar.md`, `src/world/legacy/edit.md`.

4. Add first ecology simulation slice.
   - Minimal context: no final animal entity storage is required yet; emit deterministic candidate/events first.
   - Include animal spawn, fight, carcass, grazing, and plant growth events.
   - References: `src/simulation/simulation.md`, `src/ecs/ecs.md`, `src/ecs/fixed.md`.

5. Extend fixed bridge scope.
   - Minimal context: ECS currently owns `SimClock`, `ActiveSimRegion`, and pending simulation results.
   - Add or derive a `3x3` active chunk scope for textmode without making simulation own the accumulator.
   - References: `src/ecs/fixed.rs`, `src/ecs/runtime.rs`, `src/app/fixed.rs`.

6. Implement `src/bin/new-world-textmode.rs`.
   - Minimal context: run fixed ticks, apply structured results, and print one summary per real second.
   - Format time as `YY-MM-DD HH:MM (season)` using `WorldCalendar`.
   - Format the `3x3` chunks from structured weather/surface/ecology/world-update data.
   - References: `src/app/config.rs`, `src/app/fixed.rs`, `src/simulation/mod.rs`, `src/world/legacy/calendar.rs`.

7. Verify.
   - Run `cargo fmt`.
   - Run focused tests plus `cargo test` when feasible.
   - Add tests for deterministic ecology events, surface transitions, calendar display formatting, and textmode summary formatting.

## Reporting

- List changed files and why.
- Report which docs were updated before implementation.
- Report verification commands and results.
- Commit when the implementation is complete; do not push without user permission.
