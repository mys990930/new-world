# meso

## Role

- own deterministic multi-chunk terrain guides between atlas-scale macro guidance and chunk-local realization
- derive readable local terrain accents from seed plus nearby atlas context without forcing atlas itself to change identity every few chunks
- give generation a middle-scale scaffold for features such as hill groups, escarpment bands, basins, terraces, coves, or similar multi-chunk terrain accents inside an already-classified region

## Responsibilities

- define the ownership and cache unit for meso terrain guides
- define how meso guides are derived from atlas region classification plus skeleton context
- define how meso candidates are selected from atlas/structure-conditioned weights plus deterministic randomness
- expose per-sample guide weights and directional hints that feature-owned generation resolvers can consume before hydrology and block fill
- support feature-owned resolved-instance windows with stable non-chunk ownership so chunk-local sampling can read the same multi-chunk landform objects across neighboring chunk requests
- stay deterministic and generation-order-independent
- stay broad enough to be visible across several chunks inside normal play view

## Non-Responsibilities

- atlas-scale climate or continent ownership
- primary biome or terrain-form ownership
- mountain-range or drainage topology ownership
- final `TerrainProfile` resolution
- final block placement or material fill
- whole-world precomputation

## Owned Data

- `MesoRegionCoord`
- `MesoRegion`
- `MesoGuideMap`
- `MesoGuideCell`
- `MesoGuideSample`
- feature-owned private resolved-instance caches or windows such as multi-chunk hill-cluster objects

## Public Interface

```rust
generate_meso_guides(
    meta: &WorldMeta,
    area: AtlasArea,
    fields: &AtlasFieldMap,
    structure: &AtlasStructureMap,
) -> MesoGuideMap

sample_meso_guides(
    guides: &MesoGuideMap,
    world_x: i32,
    world_z: i32,
) -> MesoGuideSample

MesoGuideMap::area(&self) -> AtlasArea
MesoGuideMap::cells(&self) -> &AtlasGrid<MesoGuideCell>
MesoGuideMap::cells_mut(&mut self) -> &mut AtlasGrid<MesoGuideCell>
debug_hill_cluster_peak_candidates(guides: &MesoGuideMap) -> Vec<HillClusterPeakCandidate>

meso_region_coord_for_atlas(coord: AtlasCoord) -> MesoRegionCoord
meso_regions_covering_area(area: AtlasArea) -> Vec<MesoRegionCoord>
```

## Scale Contract

- atlas remains the macro layer
- the initial meso target is finer than atlas and coarser than chunk-local micro detail
- the current implementation uses:
  - one meso guide cell spans `2 x 2` chunks
  - one atlas cell footprint contains `8 x 8` meso guide cells
  - one `MesoRegion` aligns to one atlas cell footprint for ownership and caching
- those exact numbers may still change later, but the layering rule should not: meso must remain a multi-chunk layer that is visibly more local than atlas

## Relationship To Other Atlas Layers

- `atlas_fields.md` owns continuous macro context such as coastness, elevation tendency, climate, and moisture
- `region.md` owns resolved biome and terrain-form archetypes
- `structure.md` owns directional macro skeleton such as mountain spines and drainage paths
- `meso.md` owns the middle-scale terrain accents that should be visible inside a small gameplay view without destroying macro or regional coherence

## Selection Model

- meso should not be a pure threshold map where atlas factors directly force a single result
- meso should also not be unconstrained random noise
- the intended model is `region/archetype constrained deterministic lottery`

### Selection Steps

1. derive meso suitability channels from nearby region archetype, selected raw atlas context, and skeleton context
2. assemble a local candidate set whose weights are biased by that context
3. use seed plus meso region/cell coordinates to run a deterministic weighted pick inside that candidate set
4. randomize the chosen feature parameters inside context-dependent bounds
5. rasterize the resulting feature instance as one or more blended guide fields rather than as a hard biome label

## Suitability Inputs

- atlas region context
  - biome family
  - terrain-form family
  - region archetype
  - resolved temperature / moisture / elevation / relief classes
- atlas skeleton context
  - distance to ridge spine
  - distance to drainage path
  - downstream progress
  - confluence proximity
- selected raw atlas context
  - inlandness / coastness
  - macro elevation
  - ruggedness
  - wetness / aridity
- derived local context
  - basin tendency
  - shoreline exposure
  - local relief budget allowed under the current region archetype

## Randomness Contract

- randomness should decide `which allowed local expression appears here`, not `whether atlas rules still matter`
- every random choice must be bounded by the current region/archetype and skeleton envelope
- examples:
  - a dry inland plain may randomly become rolling hills or a shallow basin, but not a lagoon
  - a rugged coastal zone may randomly favor cliffs, coves, or coastal terraces, but not a broad inland dune field unless the aridity signal also allows it
  - an upland ridge shoulder may randomly spawn a local escarpment band or stepped terrace form, but not a delta
- all randomness must remain deterministic from `seed + generator_version + meso_region + local_cell`

## Output Shape

- meso should emit blended guides, not final labels
- current Wave 1A channels:
  - hilliness
  - basin depth bias
  - cliff or escarpment edge bias
  - terrace band bias
  - escarpment signed-distance and heading hints
  - terrace signed-distance, spacing, and heading hints
- generation can then hand those channels to feature-owned runtime resolvers without forcing every meso landform through one shared `delta_y` formula

## Generation Integration

1. generation requests atlas raw fields for macro context
2. generation requests atlas skeleton for mountain/drainage direction
3. generation requests region classification before base heightfield solving
4. generation requests meso guides for several-chunk local accents inside that region archetype
5. generation solves a biome-aware base heightfield with river corridors as constraints
6. generation asks feature-owned meso resolvers to build target local surfaces on top of that base scaffold, then composites those results before local smoothing
7. generation applies local micro detail, final hydrology, and material fill

## Invariants

1. the same `(seed, generator_version, region)` must always produce the same meso guides
2. meso must not replace atlas macro or regional identity; it modulates local readability under that classified context
3. meso guides must span multiple chunks and remain meaningful inside a small play view
4. meso must be generated on demand and must not require whole-world precomputation
5. profile-local micro detail should decorate meso shape, not replace it as the only source of readable local terrain identity

## Current Status

- Wave 1A is implemented as deterministic atlas-owned guide generation
- the full per-feature taxonomy is now scaffolded in `meso/catalog.md` and `meso/features/*` with `launch`, `extended`, and `deferred` labels
- current generated candidates are still only `hill clusters`, `basins`, `escarpment bands`, and `terraces`
- current implemented selection still follows an atlas/structure-constrained deterministic lottery model
- runtime-wired feature-specific realization details should live in the matching `meso/features/<feature>/` folder, while `atlas/meso.rs` stays responsible for shared selection, sampling, and dispatch
- current `hill_cluster` runtime emission now has two layers:
  - atlas meso guide generation rasterizes a broad multi-chunk hill signal with several strong guide cells across a few chunks
- feature-owned chunk-side resolution must first resolve sparse multi-chunk hill objects from a stable meso-region ownership layer, then sample those same resolved objects per column during meso apply
- the current hill-cluster runtime helper keeps a broader low-amplitude shoulder/support zone than the inner hill cores so plains transition into hill country more naturally at launch scale
- the current hill-cluster runtime helper must also read a wide enough neighboring guide neighborhood that those broad hill masses stay continuous across chunk and meso-cell seams
- feature-owned chunk-side resolved windows must be deterministic from stable guide ownership alone; neighboring chunks sampling the same hill mass should not re-roll a different object graph at the seam
- generation now samples these guides per block column in the explicit post-prototype meso stage before later smoothing, but target architecture is feature-owned surface resolution rather than one shared additive deformation formula
- preview and debugging tooling may clone a `MesoGuideMap`, zero non-target channels through `cells_mut()`, and run the same feature-owned runtime on a flat baseline to inspect meso shape in isolation before blending it back onto the real prototype
- preview and debugging tooling may also query `debug_hill_cluster_peak_candidates(...)` from the same filtered guide map to visualize which local hill-guide peaks are even entering hill-cluster resolve before owner-region sparsening and per-hill resolve
- hill-cluster runtime resolve should keep low-amplitude support and direct blob-core height as separate signals so transition fill cannot silently become a synthetic peak far away from the actual guide sources
- the current chunk-side launch pass applies only the Wave 1A subset and uses each archetype's current `allowed_meso_keys` stub as a temporary runtime gate until the authoritative per-archetype matrix is locked
- candidate-family grouping notes and wave-order notes still live in `meso_candidates.md`
- target architecture update: future meso selection should become region/archetype constrained first, with raw scalar context used only as bounded secondary input

## Submodules

- `catalog.md`
- `features/features.md`
