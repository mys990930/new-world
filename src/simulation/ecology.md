# ecology

## Role

- emit deterministic chunk-scoped ecology observations for active simulation scopes
- keep animal and plant activity as structured events until ECS/entity storage exists

## Responsibilities

- consume fixed tick, world seed, and active chunk inputs
- carry each chunk's graph-first `GraphBiomeKind` into every ecology event
- emit `SimEvent::EcologyEventObserved` for textmode and later gameplay consumers
- cover first-slice event categories: animal spawn, animal fight, carcass creation, grazing, and plant growth advance

## Non-Responsibilities

- storing animal entities
- mutating world source-of-truth directly
- choosing the active chunk window
- formatting console text
- expanding surface-condition state

## Inputs

- `EcologySimInput`
- `EcologySimBundleInput`
- `EcologySimChunkInput { coord, biome: GraphBiomeKind }`
- `EcologySimConfig`

## Outputs

- `SimulationResult { subsystem: SubSystemId::Ecology, events: [...] }`
- `SimEvent::EcologyEventObserved { scope: SimSpatialScope::Chunk(...), biome: GraphBiomeKind, event }`

## Processing Scale

- the first slice runs on `ticks_per_ecology_step`
- the default cadence is one ecology step per fixed-rate second
- each active chunk emits two to three deterministic observed/candidate events for the current ecology window

## Determinism

- event count and selection are pure functions of `world_seed`, ecology window, chunk coordinate, and biome-derived policy
- input chunks are sorted by coordinate before event emission so equal sets produce stable result order
- no runtime randomness, wall-clock time, app state, or textmode formatting affects rule output

## Current Biome Policy

- water/lake cells prefer `small_fish` and reed-style forage
- wetland/coast cells prefer `wading_bird`, `reed`, or `mangrove_sapling`
- grassland/steppe/savanna cells prefer `deer`, `wolf`, and grass forage
- temperate/tropical forest cells prefer `boar`, `wolf` or `bear`, and tree forage
- boreal/subalpine cells prefer `deer`, `bear`, and conifer forage
- dry/desert cells prefer `hare`, `fox`, and shrub/cactus forage
- tundra/polar cells prefer `hare`, `bear`, and moss forage

## Related Modules

- `simulation.md`
- `../ecs/ecs.md`
- `../ecs/fixed.md`
- `../world/world.md`

## Current Implementation Notes

- this is an observer/candidate slice only
- animal spawn/fight/carcass and plant grazing/growth events are gameplay-facing data that Step 5 can hand to ECS scopes and Step 6 can format in `new-world-textmode`
- event species/plants are biome-specific candidate labels, not persistent entity archetypes yet
- final entity creation, persistent animal state, plant inventory effects, and world edits remain future work
