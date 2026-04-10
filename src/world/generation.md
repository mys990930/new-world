# generation

## Role

- Turn `WorldMeta.seed`, chunk coordinates, and atlas-scale macro inputs into deterministic `ChunkData`.
- Own chunk realization rules such as surface height and solid terrain fill.
- Consume atlas fields as input, but remain the module that decides actual block placement.

## Responsibilities

- Define the chunk-generation entry point and its deterministic contract.
- Sample atlas-scale macro environment data needed for chunk realization.
- Convert atlas fields into per-column surface elevation around a fixed sea level.
- Fill solid terrain mass into `ChunkData` for relief inspection.
- Keep generation independent from loaded-world mutation, jobs scheduling, and renderer concerns.

## Non-Responsibilities

- Owning atlas field generation itself
- Mutating live world storage
- Async worker orchestration
- Meshing
- Vegetation, trees, grass props, or ecology placement

## Inputs

- `ChunkCoord`
- `WorldMeta`
- `BlockRegistry`

## Outputs

- `ChunkData`

## Public Interface

```rust
generation::generate_chunk(
    coord: ChunkCoord,
    meta: &WorldMeta,
    registry: &BlockRegistry,
) -> ChunkData
```

## Current First-Pass Realization Contract

- Sea level is fixed at world-space `y = 0`.
- The generator treats atlas as macro input and performs block placement inside `world::generation`.
- For each `(x, z)` column in the chunk:
  1. Sample atlas-derived macro inputs and convert them into a signed surface elevation.
  2. Fill `stone` from world `y = -256` through `surface_y`.
  3. Leave everything above `surface_y` as air, including ocean space above negative-height seabeds.
- This phase intentionally does not place `water`, `grass`, `dirt`, `sand`, `mud`, `snow`, trees, or ecology.
- The purpose of this phase is to verify that atlas-driven macro relief such as sea basins, coasts, rivers, and ridges reads well in raw stone form before material layering begins.

## Processing Flow

1. Map the target chunk to the atlas neighborhood needed for macro sampling.
2. Generate atlas fields for that neighborhood.
3. Interpolate the atlas signals per block column inside the chunk.
4. Convert atlas signals into signed surface elevation.
5. Write block ids into `ChunkData`.
6. Return the finished chunk without mutating any live world state.

## Invariants

1. The same `(seed, generator_version, coord)` must always produce the same `ChunkData`.
2. `world::generation` owns block placement; `world::atlas` does not place blocks directly.
3. Sea level remains fixed at world-space `y = 0` for this generator version.
4. Blocks below world-space `y = -256` are outside the current generated volume.
5. The current generator version emits only `stone` and `air`.
6. Generation reads block meaning through `BlockRegistry`; it does not own texture or renderer policy.

## Related Modules

- `meta.md`
- `coord.md`
- `chunk.md`
- `registry.md`
- `atlas/atlas.md`
- `jobs/jobs.md`
