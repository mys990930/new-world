# generation

## Role

- Turn `WorldMeta.seed`, chunk coordinates, and atlas-scale macro inputs into deterministic `ChunkData`.
- Own chunk realization rules such as surface height and solid terrain fill.
- Consume atlas fields as input, but remain the module that decides actual block placement.
- Resolve generation-side terrain profiles from atlas weights before filling blocks.

## Responsibilities

- Define the chunk-generation entry point and its deterministic contract.
- Sample atlas-scale macro environment data needed for chunk realization.
- Resolve terrain profiles such as deep ocean, shelf, coast, plain, upland, and ridge from sampled atlas fields.
- Convert atlas fields into per-column surface elevation around a fixed sea level using blended profile surfaces rather than a single hard profile switch.
- Fill solid terrain mass into `ChunkData` for relief inspection.
- Expose deterministic debug probes for chunk/profile/surface inspection while tuning generation.
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

generation::probe_chunk(coord: ChunkCoord, meta: &WorldMeta) -> ChunkGenerationProbe
generation::probe_column(
    coord: ChunkCoord,
    local_x: u8,
    local_z: u8,
    meta: &WorldMeta,
) -> ColumnGenerationProbe
generation::sample_chunk_surface_lod(
    coord: ChunkCoord,
    step_blocks: u8,
    meta: &WorldMeta,
) -> ChunkSurfaceLodGrid
```

## Current First-Pass Realization Contract

- Sea level is fixed at world-space `y = 0`.
- The generator treats atlas as macro input and performs block placement inside `world::generation`.
- For each `(x, z)` column in the chunk:
  1. Sample and bilerp atlas-derived macro inputs from the surrounding atlas cells.
  2. Resolve a dominant generation-side `TerrainProfile` for debug and material heuristics.
  3. Blend the profile surface shapers plus block-scale deterministic relief noise into a base signed `surface_y`.
  4. Resolve a column material profile, with coast classification taking priority over river-bed classification near sea level so beaches remain visible.
  5. Optionally carve river floodplains/channels out of the base surface and assign a `water_top_y` for inland rivers or sea water.
  6. Pick a stone-core ceiling at `surface_y - random(8..=16)` from the carved final ground surface.
  7. Fill `stone` from world `y = -256` through that ceiling.
  8. Fill the layer above the stone core through `surface_y` using atlas-informed material rules:
     - deep ocean floor: `mud`
     - shallow ocean floor: `sand` / `mud`, with extra `gravel` when not river-connected
     - river headwaters: `gravel` bed, with exposed land banks still becoming `grass`
     - river middle reaches: `gravel` / `sand` bed, with exposed land banks still becoming `grass`
     - river lower reaches: `mud` / `sand` bed, with exposed land banks still becoming `grass`
     - coast: `sand`
     - desert: `sand`
     - alpine or polar terrain: `snow`
     - otherwise: `dirt`, with `grass` on the surface block
  9. Any exposed land surface that is not classified as `sand` or `snow` uses `grass` as the top block.
  10. If a column has `water_top_y`, fill `water` from `surface_y + 1` through that water top:
     - ocean/shelf columns use sea level `y = 0`
     - inland river columns can carry water above sea level
- Trees, tall grass, and ecology are still intentionally out of scope for this pass.

## Processing Flow

1. Map the target chunk to the atlas neighborhood needed for macro sampling.
2. Generate atlas fields for that neighborhood.
3. Interpolate the atlas signals per block column inside the chunk.
4. Resolve a dominant terrain profile from the sampled column context.
5. Blend the profile-specific surface functions into a single surface height.
6. Apply hydrology-aware carving and water-top resolution.
7. Write block ids into `ChunkData`.
8. Return the finished chunk without mutating any live world state.

## Invariants

1. The same `(seed, generator_version, coord)` must always produce the same `ChunkData`.
2. `world::generation` owns block placement; `world::atlas` does not place blocks directly.
3. Sea level remains fixed at world-space `y = 0` for this generator version.
4. Blocks below world-space `y = -256` are outside the current generated volume.
5. The current generator version emits layered terrain materials plus sea water and inland river water, but still no vegetation or ecology.
6. Generation reads block meaning through `BlockRegistry`; it does not own texture or renderer policy.

## Internal Submodules

- `context.md`: shared generation structs such as atlas samples, palette, and column realization
- `sampler.md`: chunk-to-atlas neighborhood lookup and bilerp sampling
- `profile.md`: generation-side terrain profile resolution
- `noise.md`: deterministic block-scale relief noise helpers
- `probe.md`: deterministic terrain inspection helpers
- `profiles/profiles.md`: profile-specific surface shaping modules
- `realize.md`: chunk fill loop and layered terrain realization

## Related Modules

- `meta.md`
- `coord.md`
- `chunk.md`
- `registry.md`
- `atlas/atlas.md`
- `jobs/jobs.md`
