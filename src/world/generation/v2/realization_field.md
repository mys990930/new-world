# realization_field

## Role

- own the continuous prototype-control solve that sits between discrete region classification and the biome-aware base heightfield
- turn atlas-cell semantic classes into a world-space realization field so terrain no longer exposes atlas rectangles through direct cell-to-heightfield parameter switches
- provide the authoritative design contract for the current realization-field stage and its next tuning steps

## Responsibilities

- consume atlas raw fields, nearby structure context, and resolved `RegionClassCell` semantics for the neighborhood around a target chunk
- derive continuous prototype-control parameters such as uplift bias, flatness, relief, wet flattening, ridge affinity, terrace tendency, and corridor susceptibility
- preserve semantic stability from region classification while allowing those control parameters to cross atlas-cell boundaries when the surrounding terrain context remains compatible
- block or weaken continuity where macro context should stay sharp, such as strong coasts, ridge divides, escarpment rims, or basin boundaries
- expose a sampleable world-space control field that prototype can query per column without caring which atlas cell originally supplied the dominant archetype
- remain deterministic, chunk-order-independent, and local-neighborhood-bounded

## Non-Responsibilities

- choosing the semantic biome / terrain-form / archetype labels themselves
- final river corridor solve
- final base-height evaluation
- meso accent placement
- final hydrology or voxel fill

## Inputs

- `ChunkCoord`
- `ChunkGenerationV2Inputs`

## Outputs

- `ChunkRealizationFieldPatch`
- `RealizationSample`

## Interface

```rust
build_chunk_realization_field_patch(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationV2Inputs,
) -> ChunkRealizationFieldPatch

sample_chunk_realization_field(
    patch: &ChunkRealizationFieldPatch,
    world_x: f32,
    world_z: f32,
) -> RealizationSample
```

## Data Model

- `RealizationSourceHint`
  - the locally anchored semantic intent derived from one classified region cell plus nearby atlas context
- `RealizationFieldNode`
  - one node on the sub-atlas control lattice
  - stores both the locally anchored source sample and the solved continuous parameter vector used by prototype
- `ChunkRealizationFieldPatch`
  - the chunk-local solved control patch including halo support so neighboring chunks agree along shared boundaries
- `RealizationSample`
  - the bilinearly sampled control vector that prototype consumes at a world-space position

## Parameter Channels

The first implementation should stay narrow and include only channels that prototype already knows how to consume well:

- `uplift_bias`
- `flatness`
- `relief_base`
- `relief_gain`
- `wet_flatten`
- `ridge_lift`
- `ridge_shoulder_lift`
- `ridge_preservation`
- `basin_affinity`
- `terrace_bias`
- `dune_bias`
- `corridor_depth_scale`
- `floodplain_width_scale`
- `meso_relief_reserve`

These are continuous generation controls, not a replacement encoding of `RegionArchetype`.

## Lattice Model

- the realization field should use a control lattice that is finer than atlas cells and coarser than per-block prototype columns
- initial target resolution:
  - one realization node per `2 x 2` chunks
  - this yields `8 x 8` realization nodes inside one `16 x 16` chunk atlas cell
- each chunk solve should include enough halo nodes that neighboring chunks sample the same solved field along shared boundaries
- current implementation:
  - `REALIZATION_NODE_CHUNK_SPAN = 2`
  - `REALIZATION_PATCH_HALO_NODES = 2`

## Source Construction

Each realization node starts from a source hint built from:

- nearby `RegionClassCell` semantic identity
- archetype-owned prototype hint deltas
- raw atlas scalar context such as macro elevation, wetness, ruggedness, coast influence, basinness, and aridity
- nearby structure context such as ridge influence or broad drainage presence

The source hint is an anchor, not the final node value.

## Continuity / Permeability Model

Realization nodes should exchange influence through edge permeability rather than through plain distance-only blur.

Permeability should increase when neighboring nodes share compatible context:

- same or nearby biome family
- same or nearby terrain-form family
- similar raw scalar context
- similar ridge / basin / coast orientation

Permeability should decrease when the macro context says a boundary should remain visible:

- marine-to-inland breaks
- strong ridge divides
- clear escarpment rims
- basin outlet walls
- severe relief or wetness discontinuities

## Solve Model

- construct source hints for all nodes in the local patch plus halo
- compute edge permeability between neighboring nodes
- run a deterministic local relaxation / diffusion solve where each node:
  - stays anchored to its own source hint
  - is pulled toward compatible neighboring node values according to permeability
- sample the solved node field in world space when prototype requests a control vector
- current implementation note:
  - the first solve uses a bounded neighborhood gather with permeability-weighted averaging plus an anchor-strength blend, rather than a multi-iteration global relaxation pass
  - permeability already considers distance, semantic compatibility, scalar-context similarity, marine/inland transitions, and basin-wall pressure

The solve target is not "blur everything"; it is "carry compatible landform intent across atlas-cell boundaries without losing macro barriers".

## Relationship To Region Classification

- region classification remains the semantic owner of biome family, terrain-form family, hydrology context, and archetype
- realization field is generation-owned and should not redefine those labels
- the realization field should therefore diffuse parameter vectors, not archetype ids

## Relationship To Corridor Solve

- corridor solve remains a separate structural constraint stage
- realization field should describe broad landform tendency before corridor carve pressure is applied
- prototype should combine:
  - realization-field samples for broad landform identity
  - corridor constraints for valley seat, floodplain, outlet, and coastal-exit pressure

## Invariants

1. deterministic for the same `(seed, generator_version, chunk, inputs)`
2. chunk-edge continuity is mandatory
3. atlas-cell rectangles must not remain directly legible in prototype simply because region classification is cell-owned
4. semantic biome identity may stay discrete while realization parameters remain continuous
5. the solve must stay local-neighborhood-bounded and not require whole-world precomputation

## Why This Exists

- the previous prototype directly blended neighboring classified atlas cells at sample time
- that softened hard steps, but still left a piecewise atlas-grid-shaped control field at large scale
- the realization field now exists specifically to remove that remaining atlas-grid imprint and let terrain context continue naturally across classified-cell boundaries

## Deferred

- exact runtime storage layout
- stronger multi-step relaxation / solve variants beyond the current bounded diffusion pass
- any final corridor-aware feedback into the realization solve
- meso coupling beyond preserving a later `meso_relief_reserve`
