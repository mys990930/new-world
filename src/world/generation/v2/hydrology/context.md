# hydrology/context

## Role

- carry shared hydrology-stage DTOs that let the orchestrator and pass modules communicate without widening the public `world::generation` API
- keep branch identity, region hydrology style signals, projected corridor geometry, and per-corridor response state in one internal contract

## Boundaries

- owns internal pass context only
- does not sample atlas fields, carve terrain, place water, or choose block materials
- remains `pub(super)` to keep the external hydrology surface stable

## Invariants

1. context values must be deterministic for the same chunk input
2. response and signal structs must stay finite before they are converted into public `HydrologyColumn` values
3. branch keys must preserve enough identity to count connected waterlines without exposing atlas internals downstream
