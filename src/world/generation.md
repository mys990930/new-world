# generation

## Role

- define the public world-generation surface during the V2 transition
- own the chunk-generation entrypoint contract, even while actual V2 realization is still incomplete
- assemble chunk-local V2 inputs from atlas raw fields, skeleton guidance, region classification, and meso ownership
- reserve V2 stage boundaries until real prototype, hydrology, and voxelization logic lands

## Responsibilities

- define the public `generate_chunk(...)` contract for future V2 realization
- assemble padded atlas field / skeleton / region / meso inputs for a chunk
- keep the V2 stage split explicit: corridors, prototype, meso apply, smoothing, hydrology, voxelize
- preserve compile-time probe and LOD data shapes while runtime diagnostic behavior is intentionally disabled
- keep generation independent from loaded-world mutation, jobs scheduling, and renderer concerns

## Planned Multi-Scale Model

- `atlas scalar macro`
  - climate, coastness, continent-ness, elevation tendency, wetness
- `atlas structure`
  - mountain-range direction, ridge skeleton, drainage direction, river topology
- `atlas region classification`
  - biome family, terrain-form family, hydrology context, region archetype
- `atlas meso guides`
  - several-chunk terrain accents such as local hill groups, escarpment bands, basins, terraces, or coastal breakup
- `generation profile families`
  - `DeepOcean`, `Shelf`, `Coast`, `Plain`, `Upland`, `Ridge`
- `biome-aware base heightfield`
  - region/archetype-owned broad terrain solve that already respects river corridors and basin outlets
- `generation local detail`
  - profile-local relief noise, smoothing, and small contour breakup
- `seasonal biome state`
  - current in-year surface state derived from region archetype, climate regime, world calendar, elevation, and hydrology
- `hydrology/material fill`
  - final river carve, water surface, sediment choice, topsoil, stone core, and block fill

## Non-Responsibilities

- Owning atlas field generation itself
- Owning atlas mountain-chain or drainage graph creation itself
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

generation::prepare_chunk_v2_inputs(
    coord: ChunkCoord,
    meta: &WorldMeta,
) -> ChunkGenerationV2Inputs
generation::build_chunk_v2_scaffold(
    coord: ChunkCoord,
    meta: &WorldMeta,
) -> ChunkGenerationV2Scaffold

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

## Current Generator Contract

- Sea level is still fixed at world-space `y = 0`.
- `generate_chunk(...)` currently exists only as a compile-only TODO stub.
- probe helpers also exist only as compile-only TODO stubs.
- the last working V1 realization stack has been intentionally removed instead of being kept as a silent fallback.
- the authoritative transition summary now lives in `status.md`.

## V2 Architecture Target

- Atlas remains the owner of macro terrain direction, but V2 also requires explicit region classification before meso and before base heightfield solving.
- Region classification should resolve stable biome and terrain-form archetypes from raw continuous fields plus skeleton context, instead of asking meso or material thresholds to decide primary local identity.
- Climate regime should be treated as a derived long-pattern class, while seasonal biome state should stay a later runtime layer that changes cover/material expression without constantly reclassifying the region archetype.
- River corridors should be defined before biome-aware base heightfield solving, so heightfield generation treats them as constraints rather than as late carve masks.
- Meso should become a constrained local-accent layer inside those already-classified regions, not the primary biome owner.
- In V2, headwaters should naturally emerge near mountain spines, passes, upland divides, and basin outlets rather than appearing as isolated wet pockets.

## Processing Flow

This flow is no longer implemented. See `status.md` for what remains from the old path and which V2 stages still need real code.

## Planned V2 Processing Flow

1. Map the target chunk to the atlas neighborhood needed for raw fields, skeleton, and region classification.
2. Generate or read the atlas raw-field window plus mountain/drainage skeleton window.
3. Resolve or sample the matching region classification window.
4. Generate or read river corridors, basin outlets, and downstream grade for the same footprint.
5. Solve a biome-aware base heightfield from region archetype plus water-corridor constraints.
6. Generate or read the matching meso guide window from region context and aligned meso regions.
7. Apply meso deformation on top of the base heightfield.
8. Smooth that surface while preserving major corridor and ridge intent, then derive local refinement signals.
9. Solve final hydrology and connected water surfaces from the pre-defined branch model.
10. Resolve region/material ownership and topsoil / sediment / cover policy.
11. Write block ids into `ChunkData`.
12. Return the finished chunk without mutating any live world state.

## Probe Scope Notes

- `probe_chunk`, `probe_column`, and `sample_chunk_surface_lod` are inspection helpers for atlas sampling and base surface reasoning.
- At the moment they are compile-only TODO stubs kept so tooling callers still build.
- Tools that still call them should be treated as disabled until V2 diagnostics are implemented.

## Invariants

1. The future V2 generator must remain deterministic for the same `(seed, generator_version, coord)`.
2. `world::generation` will own final block placement; `world::atlas` will not place blocks directly.
3. Sea level remains fixed at world-space `y = 0` for this generator version.
4. Blocks below world-space `y = -256` remain outside the intended generated volume.
5. Generation should not decide primary biome identity from material thresholds alone; it should consume atlas-owned region classification first.
6. Legacy V1 should not silently reappear as a fallback path.

## Internal Submodules

- `sampler.md`: chunk-to-atlas neighborhood lookup and bilerp sampling
- `profile.md`: retained diagnostic terrain-profile vocabulary
- `probe.md`: compile-only probe and LOD stub surface
- `status.md`: current V2 progress, retained modules, and removed legacy summary
- `v2.md`: region-first generation scaffold
- `v2/inputs.md`: atlas, skeleton, and region input assembly scaffold
- `v2/corridors.md`: river corridor and downstream-grade scaffold
- `v2/prototype.md`: biome-aware base heightfield prototype scaffold
- `v2/meso_apply.md`: meso application scaffold
- `v2/smoothing.md`: smoothing and local refinement scaffold
- `v2/hydrology.md`: final hydrology scaffold
- `v2/voxelize.md`: final material and block placement scaffold

## Related Modules

- `meta.md`
- `coord.md`
- `chunk.md`
- `registry.md`
- `atlas/atlas.md`
- `atlas/region.md`
- `atlas/meso.md`
- `surface/surface.md`
- `jobs/jobs.md`
