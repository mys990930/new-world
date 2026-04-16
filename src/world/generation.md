# generation

## Role

- Turn `WorldMeta.seed`, chunk coordinates, and atlas-scale macro inputs into deterministic `ChunkData`.
- Own chunk realization rules such as surface height and solid terrain fill.
- Consume atlas fields as input, but remain the module that decides actual block placement.
- Resolve generation-side terrain profiles from atlas weights before filling blocks.

## Responsibilities

- Define the chunk-generation entry point and its deterministic contract.
- Sample atlas-scale macro environment data needed for chunk realization.
- Consume atlas-owned region classification before solving the chunk-local base heightfield.
- Sample atlas-owned meso terrain guides after region classification and before chunk-local micro detail.
- Resolve terrain profiles such as deep ocean, shelf, coast, plain, upland, and ridge as shape operators rather than as the final biome identity.
- Keep generation-side terrain profiles distinct from both the region-classification system and the later meso accent system.
- Convert atlas fields into per-column surface elevation around a fixed sea level using blended profile surfaces rather than a single hard profile switch.
- Build a smoothed chunk-local surface field before hydrology so contour flow stays readable at block resolution.
- Rasterize atlas-owned mountain and drainage guides into chunk-local structure weights before final hydrology.
- Apply meso hill, basin, escarpment, and terrace deformation to the broad surface scaffold before final smoothing.
- Realize atlas-owned mountain spines and drainage paths into chunk-local ridge, valley, and channel geometry.
- Fill solid terrain mass into `ChunkData` for relief inspection.
- Expose deterministic debug probes for chunk/profile/surface inspection while tuning generation.
- Keep generation independent from loaded-world mutation, jobs scheduling, and renderer concerns.
- Keep probe helpers clearly separate from exact realized-chunk inspection; probes stop at surface sampling, while exact top-down previews should scan realized chunk data.

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

generation::generate_chunk_legacy(
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

- The current top-level `generation::generate_chunk(...)` alias still points at the legacy V1 generator.
- `legacy.md` now owns that explicit compatibility surface while `v2.md` owns the region-first scaffold.

- Sea level is fixed at world-space `y = 0`.
- The generator treats atlas scalar fields plus atlas-owned structure guides as macro input and performs block placement inside `world::generation`.
- Current `TerrainProfile` categories are shape families for realization and debugging; they stay distinct from the implemented multi-chunk meso guide layer.
- For each `(x, z)` column in the chunk:
  1. Sample and bilerp atlas-derived scalar macro inputs from the surrounding atlas cells.
  2. Read the matching padded atlas structure window and rasterize nearby mountain spines and drainage paths into per-column guide weights.
  3. Read the matching padded atlas meso window and sample Wave 1A hill, basin, escarpment, and terrace guides for the same column footprint.
  4. Resolve a dominant generation-side `TerrainProfile` for debug and material heuristics.
  5. Blend the profile surface shapers into a raw signed `surface_y`, then bias that scaffold with Wave 1A meso guides plus ridge/channel guide weights before smoothing the local chunk heightfield in world-space.
  6. Resolve a column material profile using warped low-frequency boundary noise, with emergent shelf columns above sea level able to collapse into coast instead of staying as dry shallow-ocean sediment.
  7. Carve river floodplains/channels out of the smoothed surface using structure-guided channel proximity first, then scalar hydrology and local concavity as secondary support, with explicit confluence nodes widening/deepening tributary joins and a branch-consistent inland waterline before assigning `water_top_y` for inland rivers or sea water.
  8. Pick a stone-core ceiling at `surface_y - random(8..=16)` from the carved final ground surface.
  9. Fill `stone` from world `y = -256` through that ceiling.
  10. Resolve column-scale surface/fill blocks, then fill the layer above the stone core through `surface_y` using atlas-informed material rules:
     - deep ocean floor: `mud`
     - shallow ocean floor: broad `sand` / `mud` / occasional `gravel` zones chosen per column from a low-frequency sediment field instead of per-block random noise
     - river headwaters: mostly `gravel` beds, but still chosen from a smooth column-scale sediment field
     - river middle reaches: smooth `gravel` / `sand` reaches rather than checkerboard block noise
     - river lower reaches: smooth `mud` / `sand` reaches
     - coast: `sand`
     - desert: `sand`
     - thermally polar terrain, or alpine terrain that is also cold enough: `snow`
     - otherwise: `dirt`, with `grass` on the surface block
11. Any exposed land surface that is not classified as river/coast/desert/snow uses `grass` as the top block.
   Warm alpine ridges are still treated as exposed land here; `alpine_factor` alone no longer forces snow cover.
  12. If a column has `water_top_y`, fill `water` from `surface_y + 1` through that water top:
     - only `DeepOcean` / `Shelf` columns get automatic sea water up to sea level `y = 0`
     - `Coast` columns stay at or above sea level and read as beach sand rather than sea-filled low pockets
     - inland river columns can carry water above sea level and are biased toward locally concave channel cores
- Trees, tall grass, and ecology are still intentionally out of scope for this pass.

## V2 Architecture Target

- Atlas remains the owner of macro terrain direction, but V2 should also introduce explicit region classification before meso and before base heightfield solving.
- Region classification should resolve stable biome and terrain-form archetypes from raw continuous fields plus skeleton context, instead of asking meso or material thresholds to decide primary local identity.
- River corridors should be defined before biome-aware base heightfield solving, so heightfield generation treats them as constraints rather than as late carve masks.
- Meso should become a constrained local-accent layer inside those already-classified regions, not the primary biome owner.
- Target flow for each chunk:
  1. sample atlas raw fields and nearby skeleton guides together
  2. resolve deterministic region classification for the same terrain footprint
  3. define river corridors, basin outlets, and downstream grade before local heightfield solving
  4. solve a biome-aware base heightfield that avoids or accepts those corridors according to region archetype
  5. sample meso guides into several-chunk hill/cliff/basin/terrace accents allowed by that archetype
  6. apply meso deformation on top of the base scaffold before local smoothing
  7. perform final hydrology using the pre-defined corridor and branch waterline model
  8. resolve region/material ownership and then voxelize blocks
- In V2, headwaters should naturally emerge near mountain spines, passes, upland divides, and basin outlets rather than appearing as isolated wet pockets.

## Processing Flow

1. Map the target chunk to the atlas neighborhood needed for macro sampling.
2. Generate atlas fields plus the matching atlas structure and meso windows for that neighborhood.
3. Interpolate scalar atlas signals per block column inside the chunk.
4. Rasterize nearby mountain spines and river paths into chunk-local structure guide weights.
5. Sample meso guides into several-chunk hill, basin, escarpment, and terrace deformation weights.
6. Resolve a dominant terrain profile from the sampled column context.
7. Blend the profile-specific surface functions into a raw surface heightfield, then bias it with meso plus ridge/channel structure before smoothing.
8. Smooth that heightfield and derive local concavity for hydrology.
9. Apply structure-aware hydrology carving and water-top resolution.
10. Resolve per-column surface/fill blocks from fill profile plus sediment fields.
11. Write block ids into `ChunkData`.
12. Return the finished chunk without mutating any live world state.

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
- They intentionally do not represent the full realized topmost visible block after hydrology, water fill, and layered block placement.
- Tools that need an exact top-down answer should scan realized `ChunkData` instead of reading probe surfaces directly.

## Invariants

1. The same `(seed, generator_version, coord)` must always produce the same `ChunkData`.
2. `world::generation` owns block placement; `world::atlas` does not place blocks directly.
3. Sea level remains fixed at world-space `y = 0` for this generator version.
4. Blocks below world-space `y = -256` are outside the current generated volume.
5. The current generator version emits layered terrain materials plus sea water and inland river water, but still no vegetation or ecology.
6. Generation reads block meaning through `BlockRegistry`; it does not own texture or renderer policy.
7. Major ridge and river direction must eventually come from atlas-owned structure, not from chunk-local random carve alone.
8. In the target architecture, generation should not decide primary biome identity from material thresholds alone; it should consume atlas-owned region classification first.

## Internal Submodules

- `context.md`: shared generation structs such as atlas samples, palette, and column realization
- `legacy.md`: explicit wrapper for the current V1 runtime generator
- `sampler.md`: chunk-to-atlas neighborhood lookup and bilerp sampling
- `profile.md`: generation-side terrain profile resolution
- `noise.md`: deterministic block-scale relief noise helpers
- `surface.md`: smoothed chunk-local surface scaffold and local concavity derivation
- `probe.md`: deterministic terrain inspection helpers
- `profiles/profiles.md`: profile-specific surface shaping modules
- `realize.md`: chunk fill loop and layered terrain realization
- `v2.md`: region-first generation scaffold

## Related Modules

- `meta.md`
- `coord.md`
- `chunk.md`
- `registry.md`
- `atlas/atlas.md`
- `atlas/region.md`
- `atlas/meso.md`
- `jobs/jobs.md`
