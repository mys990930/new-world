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
- Keep generation-side terrain profiles distinct from any future multi-chunk meso feature system.
- Convert atlas fields into per-column surface elevation around a fixed sea level using blended profile surfaces rather than a single hard profile switch.
- Build a smoothed chunk-local surface field before hydrology so contour flow stays readable at block resolution.
- Rasterize atlas-owned mountain and drainage guides into chunk-local structure weights before final hydrology.
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
- `atlas meso guides`
  - several-chunk terrain identity such as local hill groups, escarpment bands, basins, terraces, or coastal breakup
- `generation profile families`
  - `DeepOcean`, `Shelf`, `Coast`, `Plain`, `Upland`, `Ridge`
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
- The generator treats atlas scalar fields plus atlas-owned structure guides as macro input and performs block placement inside `world::generation`.
- Current `TerrainProfile` categories are shape families for realization and debugging; they are not yet the planned multi-chunk meso feature layer.
- For each `(x, z)` column in the chunk:
  1. Sample and bilerp atlas-derived scalar macro inputs from the surrounding atlas cells.
  2. Read the matching padded atlas structure window and rasterize nearby mountain spines and drainage paths into per-column guide weights.
  3. Resolve a dominant generation-side `TerrainProfile` for debug and material heuristics.
  4. Blend the profile surface shapers into a raw signed `surface_y`, then bias that scaffold with ridge/channel guide weights before smoothing the local chunk heightfield in world-space.
  5. Resolve a column material profile using low-frequency boundary noise, with emergent shelf columns above sea level able to collapse into coast instead of staying as dry shallow-ocean sediment.
  6. Carve river floodplains/channels out of the smoothed surface using structure-guided channel proximity first, then scalar hydrology and local concavity as secondary support, with explicit confluence nodes widening/deepening tributary joins before assigning `water_top_y` for inland rivers or sea water.
  7. Pick a stone-core ceiling at `surface_y - random(8..=16)` from the carved final ground surface.
  8. Fill `stone` from world `y = -256` through that ceiling.
  9. Resolve column-scale surface/fill blocks, then fill the layer above the stone core through `surface_y` using atlas-informed material rules:
     - deep ocean floor: `mud`
     - shallow ocean floor: broad `sand` / `mud` / occasional `gravel` zones chosen per column from a low-frequency sediment field instead of per-block random noise
     - river headwaters: mostly `gravel` beds, but still chosen from a smooth column-scale sediment field
     - river middle reaches: smooth `gravel` / `sand` reaches rather than checkerboard block noise
     - river lower reaches: smooth `mud` / `sand` reaches
     - coast: `sand`
     - desert: `sand`
     - thermally polar terrain, or alpine terrain that is also cold enough: `snow`
     - otherwise: `dirt`, with `grass` on the surface block
10. Any exposed land surface that is not classified as river/coast/desert/snow uses `grass` as the top block.
   Warm alpine ridges are still treated as exposed land here; `alpine_factor` alone no longer forces snow cover.
  11. If a column has `water_top_y`, fill `water` from `surface_y + 1` through that water top:
     - only `DeepOcean` / `Shelf` columns get automatic sea water up to sea level `y = 0`
     - `Coast` columns stay at or above sea level and read as beach sand rather than sea-filled low pockets
     - inland river columns can carry water above sea level and are biased toward locally concave channel cores
- Trees, tall grass, and ecology are still intentionally out of scope for this pass.

## Next Structure-Driven Revision Target

- Atlas remains the owner of macro terrain direction, and generation now reads padded structure windows and derives chunk-local guide weights from nearby mountain spines and river paths.
- Atlas is intentionally kept macro at the current scale, so the next readability pass should add a separate meso terrain layer rather than collapsing more casual terrain identity directly into atlas cells.
- The next revision should finish replacing the remaining scalar-first river logic with fully structure-first channel realization and richer river topology handling beyond the first explicit confluence pass.
- A later revision should also insert a deterministic meso layer between atlas and micro detail so features such as hill groups, cliff bands, basins, coves, or terraces can span several chunks without requiring atlas to change identity every few chunks.
- That meso layer is planned to use atlas/structure-conditioned deterministic weighted picks rather than pure thresholds or unconstrained random noise.
- Target flow for each chunk:
  1. sample atlas scalar fields and nearby structural guides together
  2. gather the matching meso guide window for the same terrain footprint
  3. rasterize mountain-chain spine segments into distance-to-ridge / along-ridge fields
  4. rasterize drainage and river segments into distance-to-channel / along-channel fields
  5. sample meso guides into several-chunk hill/cliff/basin biases
  6. build the raw surface scaffold from profile families, then apply meso deformation before local smoothing
  7. continue promoting `along-channel` and channel heading into stronger downstream-directed water-surface and stage resolution
  8. enforce connected river channels with minimum wetted width/depth, richer confluence handling, and clearer trunk/tributary continuity
  9. keep local noise as detail only, not as the source of macro ridge or river direction
- In that revision, headwaters should naturally emerge near mountain spines, passes, and upland drainage divides rather than appearing as isolated wet pockets.

## Processing Flow

1. Map the target chunk to the atlas neighborhood needed for macro sampling.
2. Generate atlas fields plus the matching atlas structure window for that neighborhood.
3. Interpolate scalar atlas signals per block column inside the chunk.
4. Rasterize nearby mountain spines and river paths into chunk-local structure guide weights.
5. Resolve a dominant terrain profile from the sampled column context.
6. Blend the profile-specific surface functions into a raw surface heightfield, then bias it with ridge/channel structure before smoothing.
7. Smooth that heightfield and derive local concavity for hydrology.
8. Apply structure-aware hydrology carving and water-top resolution.
9. Resolve per-column surface/fill blocks from fill profile plus sediment fields.
10. Write block ids into `ChunkData`.
11. Return the finished chunk without mutating any live world state.

## Planned Next Processing Flow

1. Map the target chunk to the atlas neighborhood needed for both scalar fields and structural guides.
2. Generate or read the atlas field window plus mountain/drainage structure window.
   The structure window is built from deterministic structure regions rather than from a single globally materialized graph.
3. Generate or read the matching meso guide window from atlas context and aligned meso regions.
4. Interpolate scalar atlas signals per block column.
5. Rasterize nearby mountain spines and river paths into chunk-local directional distance fields.
6. Sample meso guides into several-chunk deformation weights and local headings.
7. Build the raw surface scaffold from profile families, then apply meso deformation plus structure-aware ridge/valley terms.
8. Smooth that surface while preserving large structural direction and meso readability, then derive local concavity only as a secondary refinement signal.
9. Realize connected channels and ridge shoulders from the structural fields, while letting meso basin/terrace signals bias where broad local relief should gather.
10. Resolve per-column materials and write block ids into `ChunkData`.
11. Return the finished chunk without mutating any live world state.

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

## Internal Submodules

- `context.md`: shared generation structs such as atlas samples, palette, and column realization
- `sampler.md`: chunk-to-atlas neighborhood lookup and bilerp sampling
- `profile.md`: generation-side terrain profile resolution
- `noise.md`: deterministic block-scale relief noise helpers
- `surface.md`: smoothed chunk-local surface scaffold and local concavity derivation
- `probe.md`: deterministic terrain inspection helpers
- `profiles/profiles.md`: profile-specific surface shaping modules
- `realize.md`: chunk fill loop and layered terrain realization

## Related Modules

- `meta.md`
- `coord.md`
- `chunk.md`
- `registry.md`
- `atlas/atlas.md`
- `atlas/meso.md`
- `jobs/jobs.md`
