# corridors

## Role

- own river corridor and downstream-grade solving between region classification and biome-aware base prototype
- project atlas drainage structure plus region context into chunk-local water-corridor constraints
- give prototype a deterministic river/valley envelope before any meso deformation or final hydrology pass

## Responsibilities

- select the drainage branches and outlet context that matter for a target chunk footprint
- convert atlas-scale river-path segments into chunk-local corridor constraints
- expose broad channel-center, valley-seat, floodplain-width, and downstream-grade guidance
- preserve trunk / tributary / headwater / outlet continuity across chunk boundaries
- keep corridor solving deterministic and independent from chunk generation order
- keep basin outlets, coastal outlets, and upland headwaters visible as prototype constraints instead of late carve masks

## Non-Responsibilities

- final carved channel voxel shape
- final connected water-surface solve
- seasonal wetness/material overrides
- meso feature selection or application
- replacing the owning `RegionArchetype`

## Inputs

- `ChunkCoord`
- `ChunkGenerationInputs`
- `AtlasStructureMap`
- `RegionClassMap`
- selected `AtlasFieldMap` hydrology and elevation tendencies when structure-only data needs bounded scalar support

## Outputs

- `ChunkCorridorWindow`

## Current Interface

```rust
build_chunk_corridor_window(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationInputs,
) -> ChunkCorridorWindow

corridors::sample_corridor_axis(
    corridor: RiverCorridorConstraint,
    local_x: f32,
    local_z: f32,
) -> CorridorAxisSample
```

The initial implementation now exists in code and keeps this stage boundary:

1. `inputs` owns atlas raw fields, skeleton, and region classification.
2. `corridors` turns those into branch-local water constraints.
3. `prototype` consumes those constraints before meso.

## Current Types

- `RiverCorridorConstraint`
- `CorridorAxisSample`
- `ChunkCorridorWindow`

## Current Type Semantics

### `RiverCorridorConstraint`

- `river_id`, `kind`, `order`
  - stable branch identity carried forward for deterministic matching and debugging
- `basin_id`, `main_stem_river_id`, `parent_river_id`
  - carried drainage-tree ownership metadata inherited from `structure`
  - intended for local filtering, confluence-aware shaping, and branch-separation policy
  - `parent_river_id` may be absent when the immediate merge target lies outside the sampled structure window
- `start_x`, `start_z`, `end_x`, `end_z`
  - chunk-relative block-space endpoints for the broad corridor segment influence
  - values may sit outside the strict `0..CHUNK_EDGE` footprint so neighboring branches can still shape edge columns
- `center_x`, `center_z`
  - chunk-relative midpoint of the carried segment
  - kept as a compact summary and debug-friendly focus point
- `half_width_blocks`
  - broad prototype-scale corridor half-width, not final carved voxel-bank width
  - intended to bias valley seat, floodplain seat, and lowland opening before smoothing
- `downstream_grade_per_block`
  - macro downstream falloff used by prototype to keep longitudinal slope coherent
  - not the final water-surface quantization and not a promise about the eventual hydrology carve depth
- `downstream_cells_start`, `downstream_cells_end`
  - structure-owned branch progress carried with the segment so later stages do not derive phase from chunk-local `t`
  - used to keep corridor-axis warp and hydrology meander continuous across adjacent atlas segments

### `CorridorAxisSample`

- shared projection result for prototype, meso keep-out, smoothing preservation, and hydrology carve
- reports both the raw segment distance and the visible signed distance after branch-progress axis offset
- derives the visible axis offset from stable branch identity plus downstream progress, not from atlas cell index or chunk-local segment position
- keeps the raw corridor chord available for debugging while ensuring visible consumers do not have to paint that chord directly

### `ChunkCorridorWindow`

- owns every corridor constraint that can materially influence the target chunk or its immediate prototype margin
- should prefer a padded influence window over a strict in-chunk crop so prototype solve remains continuous near chunk borders

## Solve Inputs By Ownership

### From `structure`

- nearest river-path segments
- branch role such as headwater, tributary, trunk, or outlet reach
- confluence neighborhood
- downstream progress
- divide / pass / basin outlet context

### From `region`

- `HydrologyContext`
- `CoastalContext`
- `ElevationBand`
- `ReliefClass`
- `TerrainFormFamily`
- `RegionArchetype`

### From raw fields

- macro elevation tendency
- basinness / lake potential
- river-flow tendency
- wetness / aridity only as bounded support signals, not as a replacement for structure or region ownership

## Processing Direction

1. gather the padded structure and region windows already assembled for the target chunk
2. select the drainage branches whose corridor influence overlaps the target chunk or its prototype margin
3. classify each relevant reach as headwater, tributary, trunk, basin outlet, inland floodplain reach, or coastal outlet reach
4. carry each selected reach forward as a broad block-space segment, not as a single point sample
5. derive longitudinal downstream grade from the segment's own downstream span and intrinsic midpoint context instead of from a chunk-local projection distance
6. derive corridor half-width from branch role, downstream progress, and bounded region context
7. emit `RiverCorridorConstraint` samples that prototype can consume without re-reading atlas structure directly
8. provide `sample_corridor_axis(...)` as the shared conversion from raw constraint to visible branch-continuous distance field

## Region Interaction Contract

- corridors come after region classification and may use region output as a constraint filter
- strong `HydrologyContext` from region classification is allowed to promote wetland, floodplain, basin, delta, or outlet-style corridor treatment during launch
- corridors must not rewrite the primary `RegionArchetype`; they only constrain where water-shaped prototype structure can appear inside or across those regions
- if region and structure disagree, structure owns long-range branch continuity while region owns the surrounding terrain identity and allowable corridor expression

## Prototype Integration Contract

- prototype should treat corridor constraints as pre-meso terrain-shape constraints
- the corridor stage is expected to define:
  - where the broad valley seat belongs
  - which direction downstream lowering must follow
  - where basin outlets or coastal exits must stay open
- prototype must not need to rediscover branch continuity from raw atlas fields once a `ChunkCorridorWindow` exists
- corridor constraints should remain broad and smooth enough that later smoothing can refine them without erasing the intended drainage network
- corridor constraints must remain broad guide envelopes. Consumers should use `sample_corridor_axis(...)` instead of direct segment projection when deriving visible valleys, keep-outs, smoothing protection, or water carve masks.
- raw segment endpoints may remain in the DTO for topology/debugging, but the visible axis must come from branch-progress sampling so segment boundaries do not become repeated straight/capsule artifacts.

## Invariants

1. the same `(seed, generator_version, chunk)` must always produce the same `ChunkCorridorWindow`
2. branch continuity must not depend on chunk generation order
3. corridor constraints must be strong enough to keep headwaters near upland structure and outlets near valid basin or coast exits
4. corridor solving must not silently degrade into isolated wet pockets that ignore drainage graph continuity
5. corridor width and downstream-grade guidance should stay smooth across neighboring chunks even when the owning branch crosses chunk borders
6. corridors should inherit a non-crossing launch-scale drainage tree from `structure`; broad main stems may merge at confluences, but they should not appear as independent crossing corridors in inland chunks
7. visible river, valley, and outlet shape must be produced by prototype/hydrology warping and blending over corridor constraints, not by directly painting the raw corridor segment geometry
8. downstream branch phase must stay continuous across adjacent carried segments for the same branch

## Notes

- an initial deterministic solve now projects nearby drainage segments into a bounded per-chunk corridor set
- the current solver keeps every segment whose expanded world-space bounds overlap the chunk prototype margin instead of truncating to a tiny top-N subset
- the carried segment set now assumes `structure` has already resolved inland large-river crossing conflicts by truncating subordinate branches into junctions; corridor solve should preserve that topology rather than reintroduce crossing ownership locally
- corridor constraints now also preserve the sampled drainage ownership chain from `structure`, so downstream hydrology can tell whether a nearby branch is the same main stem, a tributary merge, or an unrelated corridor overlap
- segment selection now uses a conservative prototype-support radius derived from the carried corridor half-width, so neighboring chunks keep the same branch available while the broad corridor envelope can still influence edge columns
- corridor scalar support now rides on canonical atlas field assembly from `sampler`, so the same branch no longer widens or steepens just because a neighboring chunk requested a different temporary raw-field window
- corridor width and downstream-grade signals are now intrinsic to the carried segment, so neighboring chunks that see the same branch keep the same broad water-envelope interpretation
- prototype now consumes corridor geometry as segment endpoints with circular endcaps rather than treating every corridor as a single radial point influence
- prototype consumption is now wired through the base-heightfield solve, so corridor continuity issues should be debugged at the shared branch/window boundary rather than at a missing integration seam
