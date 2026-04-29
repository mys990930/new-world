# hydrology/floodplain_bench

## Role

- shape broader channel-adjacent benches and shelves that soften the transition from channel to surrounding terrain
- model floodplain and bank-shelf relief separately from the main incised channel

## Boundaries

- may return lift/softening terms that modify channel carve layers
- does not place standing water or gravel materials
- does not own point bars; those are handled by `bars`

## Invariants

1. benches should blend water influence into surrounding terrain without hard carve edges
2. bench shaping must remain deterministic across chunk seams
3. inside-bend alignment can be shared with bar generation but must not itself imply gravel material
