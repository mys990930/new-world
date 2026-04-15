# meso

## Role

- own deterministic multi-chunk terrain guides between atlas-scale macro guidance and chunk-local realization
- derive readable local terrain identity from seed plus nearby atlas context without forcing atlas itself to change identity every few chunks
- give generation a middle-scale scaffold for features such as hill groups, escarpment bands, basins, terraces, coves, or similar multi-chunk terrain accents

## Responsibilities

- define the ownership and cache unit for meso terrain guides
- define how meso guides are derived from atlas scalar fields and atlas structure context
- expose per-sample guide weights and directional hints that generation can consume before hydrology and block fill
- stay deterministic and generation-order-independent
- stay broad enough to be visible across several chunks inside normal play view

## Non-Responsibilities

- atlas-scale climate or continent ownership
- mountain-range or drainage topology ownership
- final `TerrainProfile` resolution
- final block placement or material fill
- whole-world precomputation

## Planned Owned Data

- `MesoRegionCoord`
- `MesoRegion`
- `MesoGuideMap`
- `MesoGuideCell`
- `MesoGuideSample`

## Planned Public Interface

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

## Planned Scale Contract

- atlas remains the macro layer
- the initial meso target is finer than atlas and coarser than chunk-local micro detail
- the current planning assumption is:
  - one meso guide cell spans `2 x 2` chunks
  - one atlas cell footprint contains `8 x 8` meso guide cells
  - one `MesoRegion` initially aligns to one atlas cell footprint for ownership and caching
- those exact numbers may still change, but the layering rule should not: meso must remain a multi-chunk layer that is visibly more local than atlas

## Relationship To Other Atlas Layers

- `atlas_fields.md` owns scalar macro context such as coastness, elevation tendency, climate, and moisture
- `structure.md` owns directional macro skeleton such as mountain spines and drainage paths
- `meso.md` owns the middle-scale terrain identity that should be visible inside a small gameplay view without destroying macro coherence

## Generation Integration

1. generation requests atlas scalar fields for macro context
2. generation requests atlas structure for mountain/drainage direction
3. generation requests meso guides for several-chunk terrain identity
4. generation resolves profile families from macro context
5. generation applies meso deformation to the profile scaffold
6. generation applies local micro detail, smoothing, hydrology, and material fill

## Invariants

1. the same `(seed, generator_version, region)` must always produce the same meso guides
2. meso must not replace atlas macro identity; it modulates local readability under that macro context
3. meso guides must span multiple chunks and remain meaningful inside a small play view
4. meso must be generated on demand and must not require whole-world precomputation
5. profile-local micro detail should decorate meso shape, not replace it as the only source of readable local terrain identity

## Current Status

- this layer is not implemented yet
- current `TerrainProfile` and profile-local surface functions partially cover some of this visual territory, but only as shape-family logic, not as a true multi-chunk terrain guide system
