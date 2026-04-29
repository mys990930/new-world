# generation

## Role

- define the public world-generation surface
- own the chunk-generation entrypoint contract for the active region-first pipeline
- assemble chunk-local generation inputs from atlas raw fields, skeleton guidance, region classification, and meso ownership
- reserve explicit generation stage boundaries for prototype, hydrology, surface resolve, and voxelization work

## Responsibilities

- define the public `generate_chunk(...)` contract for active generation realization
- assemble padded atlas field / skeleton / region / meso inputs for a chunk
- keep the generation stage split explicit: realization field, corridors, prototype, meso surface resolve, smoothing, hydrology, voxelize
- preserve compile-time probe and LOD data shapes while runtime diagnostic behavior is intentionally disabled
- keep generation independent from loaded-world mutation, jobs scheduling, and renderer concerns

## Planned Multi-Scale Model

- `atlas scalar macro`
  - climate, coastness, continent-ness, elevation tendency, wetness
- `atlas structure`
  - mountain-range direction, ridge skeleton, drainage direction, river topology
- `atlas region classification`
  - biome family, terrain-form family, hydrology context, region archetype
- `generation realization field`
  - continuous prototype-control parameters derived from classified region semantics plus nearby atlas context
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
- `final hydrology carve`
  - connected river/lake/wetland solve plus the final post-smoothing terrain carve for channels, floodplains, basins, and outlets
- `material/block fill`
  - sediment choice, topsoil, stone core, water voxel fill, and final block placement from hydrology plus surface policy

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
generation::generate_chunk_from_generation_inputs(
    inputs: &ChunkGenerationInputs,
    registry: &BlockRegistry,
) -> ChunkData
generation::build_chunk_generation_voxelization_plan(
    inputs: &ChunkGenerationInputs,
) -> VoxelizationPlan
generation::generate_chunk_from_voxelization_plan(
    coord: ChunkCoord,
    plan: &VoxelizationPlan,
    registry: &BlockRegistry,
) -> ChunkData

generation::prepare_chunk_generation_inputs(
    coord: ChunkCoord,
    meta: &WorldMeta,
) -> ChunkGenerationInputs
generation::chunk_generation_input_area(coord: ChunkCoord) -> AtlasArea
generation::ChunkGenerationInputCache
generation::build_chunk_generation_scaffold(
    coord: ChunkCoord,
    meta: &WorldMeta,
) -> ChunkGenerationScaffold
generation::build_chunk_realization_field_patch(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationInputs,
) -> ChunkRealizationFieldPatch
generation::build_chunk_base_heightfield_prototype(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationInputs,
    realization_field: &ChunkRealizationFieldPatch,
    corridor_window: &ChunkCorridorWindow,
) -> BaseHeightfieldPrototype
generation::build_chunk_meso_applied_prototype(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationInputs,
    corridor_window: &ChunkCorridorWindow,
    prototype: &BaseHeightfieldPrototype,
) -> MesoAppliedPrototype
generation::build_chunk_smoothed_prototype(
    chunk: ChunkCoord,
    corridor_window: &ChunkCorridorWindow,
    meso: &MesoAppliedPrototype,
) -> SmoothedPrototype
generation::build_chunk_hydrology_solve(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationInputs,
    corridor_window: &ChunkCorridorWindow,
    smoothed: &SmoothedPrototype,
) -> HydrologySolve
generation::build_chunk_voxelization_plan(
    chunk: ChunkCoord,
    surface: &ChunkSurfacePlan,
) -> VoxelizationPlan

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
- `generate_chunk(...)` now runs the initial end-to-end current generation path through inputs, realization field, corridors, prototype, meso apply, smoothing, hydrology, surface resolve, and voxelization.
- `generate_chunk_from_generation_inputs(...)` follows the same end-to-end path after the current generation atlas / region / meso input bundle has already been assembled.
- `build_chunk_generation_voxelization_plan(...)` runs the expensive `x/z` terrain and surface stages through voxelization-plan assembly without committing a specific vertical chunk.
- `generate_chunk_from_voxelization_plan(...)` writes a requested `ChunkCoord` from an already-resolved plan, allowing create-world style stack generation to reuse one surface solve across many `y` chunks.
- Multi-chunk tooling may reuse one generation input bundle for all chunks that share `chunk_generation_input_area(...)`; this must not change generated terrain because the reused data is the same deterministic atlas-area input that `prepare_chunk_generation_inputs(...)` would build for each chunk.
- Multi-y tooling should solve the generation surface / voxelization plan once per `x/z` stack and only repeat the final y-specific voxel fill.
- probe helpers also exist only as compile-only TODO stubs.
- no alternate legacy terrain path is kept as a silent fallback.
- the authoritative status summary lives in `status.md`.

## Architecture Target

- Atlas remains the owner of macro terrain direction, while generation requires explicit region classification before meso and before base heightfield solving.
- Region classification should resolve stable biome and terrain-form archetypes from raw continuous fields plus skeleton context, instead of asking meso or material thresholds to decide primary local identity.
- Climate regime should be treated as a derived long-pattern class, while seasonal biome state should stay a later runtime layer that changes cover/material expression without constantly reclassifying the region archetype.
- Region classification should no longer feed prototype as a direct per-cell parameter switch. Generation should first derive a continuous realization field that carries flatness, relief, ridge, terrace, and wetness biases across atlas-cell boundaries.
- River corridors should be defined before biome-aware base heightfield solving, so heightfield generation treats them as constraints rather than as late carve masks.
- Meso should become a constrained local-accent layer inside those already-classified regions, not the primary biome owner.
- Headwaters should naturally emerge near mountain spines, passes, upland divides, and basin outlets rather than appearing as isolated wet pockets.
- No generation stage may treat an internal ownership lattice or skeleton segment as a final visible mask. Atlas-cell rectangles, structure-region seams, guide-cell squares, long straight segment cuts, repeated smooth S-curves, or uniform terrace/river bands are regressions even when chunk seams remain technically continuous.
- Discrete semantic ownership must be converted into continuous control fields for shape, feature-owned resolved objects for meso, branch-stable warped masks for hydrology, and deterministic displaced boundaries for surface material choice.

## Processing Flow

The active implementation follows the stage order below; `status.md` tracks which stages are complete, initial, or still tuning-heavy.

## Processing Flow

1. Map the target chunk to the atlas neighborhood needed for raw fields, skeleton, and region classification.
2. Generate or read the atlas raw-field window plus mountain/drainage skeleton window.
3. Resolve or sample the matching region classification window.
4. Build a continuous realization field over the same neighborhood so prototype does not sample raw atlas-cell archetype labels directly.
5. Generate or read river corridors, basin outlets, and downstream grade for the same footprint.
6. Solve a biome-aware base heightfield from realization-field control samples plus water-corridor constraints.
7. Generate or read the matching meso guide window from region context and aligned meso regions.
8. Resolve feature-owned meso surfaces on top of the base heightfield and blend them back into the prototype baseline.
9. Smooth that surface while preserving major corridor and ridge intent, then derive local slope / concavity refinement signals.
10. Solve final hydrology, connected water surfaces, and hydrology-driven terrain carve from the pre-defined branch model.
11. Resolve semantic region ownership, hydrology-aware sediment, and topsoil / cover policy through coherent displaced material boundaries that hide atlas-cell and guide-cell boundaries while preserving a deterministic owner for gameplay/query surfaces.
12. Write block ids into `ChunkData`.
13. Return the finished chunk without mutating any live world state.

## Probe Scope Notes

- `probe_chunk`, `probe_column`, and `sample_chunk_surface_lod` are inspection helpers for atlas sampling and base surface reasoning.
- At the moment they are compile-only TODO stubs kept so tooling callers still build.
- Tools that still call them should be treated as disabled until generation diagnostics are implemented.

## Invariants

1. The generator must remain deterministic for the same `(seed, generator_version, coord)`.
2. `world::generation` will own final block placement; `world::atlas` will not place blocks directly.
3. Sea level remains fixed at world-space `y = 0` for this generator version.
4. Blocks below world-space `y = -256` remain outside the intended generated volume.
5. Generation should not decide primary biome identity from material thresholds alone; it should consume atlas-owned region classification first.
6. Removed legacy paths should not silently reappear as fallback generation.
7. Visible terrain and surface materials must not expose atlas-cell, meso-cell, structure-region, or raw segment geometry as large artificial rectangles, straight lines, repeated arcs, or uniform bands.

## Stage Contract Notes

- `corridors.md` defines the river-corridor contract that feeds the base-heightfield solve.
- `prototype.md` is the authoritative design for the base-heightfield solve stage.
- `hydrology.md` owns the post-smoothing connected water solve and the final terrain carve driven by that water structure.
- `voxelize.md` should consume hydrology output rather than inventing a second late carve.
- later generation stages should treat prototype output as the broad landform source of truth, not as a late convenience mask.

## Internal Submodules

- `sampler.md`: chunk-to-atlas neighborhood lookup and bilerp sampling
- `profile.md`: retained diagnostic terrain-profile vocabulary
- `probe.md`: compile-only probe and LOD stub surface
- `status.md`: current generation progress, stage ownership, and remaining work
- `inputs.md`: atlas, skeleton, and region input assembly scaffold
- `realization_field.md`: continuous prototype-control field derived from semantic region classes and sampled by prototype before corridor shaping
- `corridors.md`: river corridor and downstream-grade scaffold
- `prototype.md`: biome-aware base heightfield prototype scaffold
- `meso_apply.md`: meso surface-resolution and compositing scaffold
- `smoothing.md`: smoothing and local refinement scaffold
- `hydrology.md`: final hydrology scaffold
- `voxelize.md`: final material and block placement scaffold

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
