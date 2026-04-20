# meso

## Role

- own deterministic multi-chunk terrain guides between atlas-scale macro guidance and chunk-local realization
- derive readable local terrain accents from seed plus nearby atlas context without forcing atlas itself to change identity every few chunks
- give generation a middle-scale scaffold for features such as hill groups, escarpment bands, basins, terraces, coves, or similar multi-chunk terrain accents inside an already-classified region

## Responsibilities

- define the ownership and cache unit for meso terrain guides
- define how meso guides are derived from atlas region classification plus skeleton context
- define how meso candidates are selected from atlas/structure-conditioned weights plus deterministic randomness
- expose per-sample guide weights and directional hints that generation can consume before hydrology and block fill
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
- generation can then consume those channels without having to know which exact authoring name originally produced them

## Generation Integration

1. generation requests atlas raw fields for macro context
2. generation requests atlas skeleton for mountain/drainage direction
3. generation requests region classification before base heightfield solving
4. generation requests meso guides for several-chunk local accents inside that region archetype
5. generation solves a biome-aware base heightfield with river corridors as constraints
6. generation applies meso deformation on top of that base scaffold before local smoothing
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
  - atlas meso guide generation rasterizes a shared clustered hill envelope with several strong guide cells across a few chunks
  - feature-owned chunk-side macro-lobe-chain resolution turns those strong neighboring guide cells into several nearby asymmetric hills with visible saddles during meso apply
- the current hill-cluster runtime helper also keeps a broader shoulder envelope than the inner hill cores so plains transition into hill country more naturally at launch scale
- generation now samples these guides per block column in the explicit post-prototype meso stage before later smoothing
- the current chunk-side launch pass applies only the Wave 1A subset and uses each archetype's current `allowed_meso_keys` stub as a temporary runtime gate until the authoritative per-archetype matrix is locked
- candidate-family grouping notes and wave-order notes still live in `meso_candidates.md`
- target architecture update: future meso selection should become region/archetype constrained first, with raw scalar context used only as bounded secondary input

## Submodules

- `catalog.md`
- `features/features.md`
