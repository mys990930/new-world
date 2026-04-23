# fixed

## Role

- own the ECS-side fixed-phase bridge to simulation
- decide which regions and subsystems should run eagerly on each fixed tick

## Owned Data

### SimClock

- logical simulation tick index
- fixed-step metadata needed on the ECS side

### ActiveSimRegion

- the region and subsystem scope that should run this tick
- typically centered on player-relevant active gameplay space

### SimulationControlState

- subsystem enable/disable state
- catch-up and throttling metadata

### PendingSimulationResults

- simulation results that have not yet been forwarded into world/jobs follow-up handling

## Inputs

- fixed-tick execution points chosen by app
- player position and related active-area context
- chunk meta-state
- completed simulation results
- world calendar and environmental context when deciding what nearby regions need eager updates

## Outputs

- simulation requests
- world-edit candidates
- dirty chunk / remesh / save follow-up requests
- active region envelopes for time/season/weather progression

## Public Interface

```rust
EcsRuntime::run_fixed_update()
EcsRuntime::sim_clock() -> SimClock
EcsRuntime::active_sim_region() -> ActiveSimRegion
EcsRuntime::enqueue_simulation_results(results)
EcsRuntime::drain_pending_simulation_results() -> Vec<SimulationResult>
```

## Process

1. app requests a fixed-phase step
2. ECS computes the active region and subsystem scope for this tick
3. ECS forwards simulation requests or collects direct simulation results
4. ECS converts those results into intermediate state for world/jobs follow-up handling

## State Transition Rules

- frame update and fixed update stay separate
- time/season/weather progression rules only advance during fixed phase
- the order in which same-tick results are applied must remain deterministic
- active-region calculation may limit eager simulation, while far-away regions rely on world-owned deferred state

## Invariants

- the fixed-timestep accumulator remains app-owned
- ECS fixed flow orchestrates simulation requests but does not implement the underlying simulation rules itself
- world mutations must still go through world-owned APIs/results
- ECS may choose which nearby areas should receive eager environmental updates, but it does not own the authoritative calendar/season state

## Non-Responsibilities

- accumulator ownership
- low-level simulation rule computation
- jobs execution
- renderer draw

## Related Modules

- `runtime.rs` runs the fixed schedule entry points
- `chunk.rs`, `jobs.rs`, `world`, and `simulation` meet here
- `player.rs` state is one of the key inputs for active-region selection
- `../simulation/time.md` defines the time/season/weather rule side that this bridge should call into

## Current Implementation Notes

- the current fixed schedule only advances `SimClock` and refreshes a player-centered `ActiveSimRegion`
- the current default active region is a single atlas cell around the local player
- `PendingSimulationResults` now exists as the ECS-owned handoff buffer from fixed simulation into app/world follow-up handling
- only the `time` simulation subsystem is wired in the current slice
