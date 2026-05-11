# new-world-textmode

## Role

- Run a continuously refreshing fixed-tick text observer for simulation/world/ECS contracts.
- Format structured state into console text without moving presentation strings into core modules.
- Redraw a box-drawing `3x3` chunk grid once per real second until the user exits with `Ctrl+C`.

## Command

```bash
cargo run --bin new-world-textmode -- [--seed <u64>] [--seconds <u32>] [--ticks-per-second <u32>]
```

Optional center flags:

- `--center-chunk-x <i32>`
- `--center-chunk-y <i32>`
- `--center-chunk-z <i32>`

## Output

- One refreshed console frame per real second.
- Header time uses `YY-MM-DD HH:MM (season)`.
- Chunk cells are drawn inside a box-drawing grid using the ECS `ActiveChunkObserverScope` `3x3` window.
- Each chunk cell includes:
  - cell biome sampled from graph-first `GraphMacroMap.biomes`, matching `biome_cell_inspector`
  - current chunk weather scalar state from world runtime state, for example
    `weather : Cloudy temp=0.62 moist=0.44 cloud=0.71 rain=0.18`
  - world-owned surface condition
  - multiple ecology events from structured `SimEvent`
  - world update records observed while applying simulation results, applying chunk weather updates, or realizing temporary chunks

## Boundaries

- This binary may realize temporary empty chunks for observer ergonomics.
- Simulation rule meaning remains in `simulation`.
- Source-of-truth state remains in `world`.
- Weather simulation input is built from graph-first biome samples, graph biome context, previous
  world-owned chunk weather, and already-known neighbor chunk weather.
- Chunk scope selection remains ECS-owned.
- `--seconds` exists for smoke tests and demos; without it the binary runs until `Ctrl+C`.
