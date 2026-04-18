# prototype

## Role

- own the biome-aware base heightfield solve before meso deformation
- turn region archetype plus corridor constraints into the broad landform that later stages refine
- remain the authoritative design contract for the base-heightfield solve stage

## Responsibilities

- convert `RegionArchetype` and `ChunkCorridorWindow` into a deterministic broad-shape prototype
- sample atlas scalar inputs continuously in world/block space instead of stepping the whole chunk on atlas-cell boundaries
- blend local region-family context near atlas boundaries without introducing a chunk-wide family step
- preserve valley seats, floodplain openings, basin outlets, and coastal exits already established by corridor solve
- aggregate repeated corridor segment responses per river branch so adjacent segments do not stack into artificial trench walls
- inject deterministic subchunk relief bands and ripple-sized height changes so low-relief terrain still reads clearly in the quarter-view camera
- solve broad terrain on a shared continuity tile rather than treating every chunk edge as an independent solve boundary
- obey the border-anchor contract from `continuity.md` so later stages inherit a stable height boundary
- allocate a remaining relief budget for later meso, smoothing, and hydrology passes
- keep the landform identity readable before local accents are added

## Non-Responsibilities

- final river carve
- final hydrology network solve
- material, sediment, or block fill decisions
- meso accent placement
- local smoothing noise
- vegetation or seasonal cover decisions
- branch discovery from raw atlas fields alone

## Inputs

- `ChunkCoord`
- `ChunkGenerationV2Inputs`
- `ChunkCorridorWindow`
- later target: shared tile bounds and stage anchor inputs from `continuity.md`

## Outputs

- `BaseHeightfieldPrototype`

## Current Interface

```rust
build_chunk_base_heightfield_prototype(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationV2Inputs,
    corridor_window: &ChunkCorridorWindow,
) -> BaseHeightfieldPrototype
```

## Data Model

- `PrototypeColumn`
- `BaseHeightfieldPrototype`

### PrototypeColumn

- one chunk-local column in the prototype solve grid
- stores the broad target height for the column in world-space block coordinates
- stores how much vertical variation remains available for later stages
- may store future hints for valley, plateau, slope, shelf, or basin behavior without committing to final carve detail

### BaseHeightfieldPrototype

- the solved chunk-local base terrain scaffold before meso
- one `PrototypeColumn` per chunk column at the current chunk surface resolution
- deterministic for the same `(seed, generator_version, coord, inputs, corridor_window)`
- preserves chunk-edge continuity by treating neighboring atlas context as part of the same solve neighborhood

## Planned Internal Solve Boundary

The current code-facing entrypoint is still chunk-oriented, but the design target is a shared tile solve:

- solve a haloed continuity tile once
- obey the tile's border-anchor set
- crop chunk interiors from the shared tile result

`continuity.md` owns the tile and anchor contract. This document owns how prototype uses that contract.

## Coordinate Space

- columns are chunk-relative and aligned to the chunk's block-grid footprint
- `base_height` is expressed in world-space block Y, not in atlas-cell units
- `relief_budget` is a nonnegative budget in block units describing how much local up/down shape can still be introduced later without erasing the broad form
- corridor segment endpoints and centers may lie outside the strict chunk bounds, but the influence on prototype columns is still evaluated in chunk-local block space
- atlas scalar sampling is evaluated from block-center world coordinates through a fractional atlas lookup, while region identity is blended from the neighboring classified atlas cells around the same sample point
- when prototype moves onto continuity tiles, shared border columns are owned by anchor samples expressed in the same world/block coordinate system

## Prototype Semantics

- `base_height`
  - the provisional solid-surface target for the column before meso and later detail
  - not the final exposed surface after smoothing, hydrology, or voxelization
  - may represent a plateau top, valley floor, shelf break, ridge shoulder, basin bowl, dune crest neighborhood, or coastal rim depending on family
- `relief_budget`
  - how much local vertical texture the later pipeline may still spend in the column
  - should shrink when the broad landform is already sharp, exposed, or corridor-sensitive
  - should stay larger in plains, terraces, and other areas that still need room for meso and smoothing to do visible work

## Launch Policy Families

- the prototype solve should group launch archetypes into broad shape families instead of handling each archetype as a unique algorithm
- families should be stable enough that later extended archetypes can slot into the same structure without rethinking the solve boundary

### Marine and Coastal Edge

- archetypes: `oceanic_shelf`, `sandy_beach_plain`, `coastal_cliffland`
- shape intent:
  - shelf stays broad and low with a marine platform identity
  - beach stays gently graded with a shoreline apron
  - cliffland keeps a nearshore wall or steep escarpment presence
- corridor interaction:
  - coastal exits should stay open
  - shoreward spill and outlet room should not be pinched off by inland shape

### Wet and Cold Lowland

- archetypes: `cold_wet_lowland`, `tundra_plain`
- shape intent:
  - keep broad low relief, saturated flats, or cold exposed flats readable
  - preserve room for shallow basins, wet margins, and freeze-thaw flattening
- corridor interaction:
  - valley seats should widen into wet lowland openings instead of becoming narrow troughs
  - basin outlets should remain readable and not collapse into closed depressions

### Temperate and Savanna Plain

- archetypes: `temperate_plain`, `savanna_plain`
- shape intent:
  - broad readable plains with modest swales and occasional hill clusters
  - keep the prototype open enough for meso to add secondary interest later
- corridor interaction:
  - river corridors should carve gentle valley seats and floodplain openings, not canyon walls
  - broad landform should stay mostly flat away from the corridor spine

### Temperate and Tropical Hill Country

- archetypes: `temperate_hills`, `tropical_rainforest_hills`
- shape intent:
  - rolling hill-country frames with ridge shoulders and valley benches
  - enough vertical structure to support later ravines or terrace accents
- corridor interaction:
  - corridor centers should anchor valleys, shoulder breaks, and drainage alignments
  - ridge continuity should survive even when valleys widen around outlets

### Temperate Plateau and Escarpment Edge

- archetype: `temperate_plateau`
- shape intent:
  - tableland top surfaces with clear edge intent and stepped descents
  - broad plateau identity should come from prototype, not from meso decoration
- corridor interaction:
  - outlets and edge breaches should read as controlled drops through the plateau boundary
  - corridor constraints should protect the plateau crest from being flattened away

### Steppe and Arid Plain

- archetypes: `steppe_plain`, `desert_plain`
- shape intent:
  - wide open floors with stronger exposure, subtle pediment behavior, and long views
  - keep arid landforms readable without depending on later dune or ravine accents
- corridor interaction:
  - drains should become open runoff lanes or shallow valleys, not dense wet floodplains
  - relief budget should stay available for later aeolian or erosional accents

### Arid Dune Body

- archetype: `desert_dune_field`
- shape intent:
  - prototype owns the dominant dune-body identity
  - meso is intentionally sparse here and should not recreate the primary dune field
- corridor interaction:
  - any drainage influence should be treated as weak and broad
  - inland prototype should stay dry, open, and landform-led rather than hydrology-led

### Polar and Alpine High Relief

- archetype: `glaciated_alpine`
- shape intent:
  - icefield and snow-upland identity with strong macro control
  - broad troughs, bowls, and cold upland shoulders should come from prototype
- corridor interaction:
  - headwaters, passes, and basin outlets should stay legible
  - relief budget should be conserved for later glacial and snow-specific refinement

## Solve Pipeline

1. sample each column's atlas scalar inputs continuously in world/block space and gather the neighboring classified region cells around that same sample point
2. read canonical corridor branch fields and the shared border-anchor set for the surrounding continuity tile
3. blend nearby region-family context so atlas-cell boundaries transition gradually instead of introducing abrupt base-height steps
4. classify corridor response as valley-seat, floodplain, basin-outlet, coastal-exit, or ridge-pressure influence
5. establish a broad target frame for the blended local family context, such as shelf, plain, lowland, hill country, plateau, arid floor, dune body, or alpine upland
6. collapse repeated river-path segment responses by branch so a long river does not over-carve where adjacent segments overlap the same column
7. add deterministic subchunk ripple and terrace-like variation that stays continuous in world space and does not depend on chunk order
8. solve the tile's column heights with deterministic falloff from corridor constraints, then force or strongly blend the border band toward the shared anchor samples
9. distribute relief budget to preserve room for later meso and smoothing without changing the broad identity
10. emit the completed tile result and crop one `PrototypeColumn` per chunk column for the requested chunk

## Corridor Influence Rules

- valley seats
  - lower the broad target height along the carried corridor segment
  - widen the floor so the prototype does not collapse into a pinched trench
- floodplain openings
  - keep the floor broad and shallow
  - reduce relief budget near the active opening so later stages do not overbuild banks too early
- basin outlets
  - preserve a passable spill path
  - avoid raising an outlet crest into a closed bowl
- coastal exits
  - keep shoreward transitions open and readable
  - prevent inland landform pressure from sealing the coast off from the sea
- ridge pressure
  - allow ridge continuity to remain visible even when neighboring valleys are widened
  - do not erase high-ground identity just because a nearby corridor passes through the chunk

## Invariants

1. deterministic for the same inputs and world seed
2. seam continuity across chunk boundaries is mandatory
3. corridor continuity is mandatory
4. atlas-cell boundaries must not introduce artificial base-height steps just because the chunk crossed into a new input window
5. shared tile borders must agree with their border-anchor samples
6. only broad-shape information belongs here
7. final hydrology is deferred
8. material and block decisions are deferred
9. meso accents are deferred
10. local smoothing detail is deferred, but prototype may still carry deterministic subchunk banding when that is needed to keep terrain legible at gameplay camera scale

## Deferred

- final river-bed depth and water surface solve
- sediment, topsoil, stone, and block material choice
- small-scale contour breakup
- weathering or seasonal surface state
- vegetation cover or ecology placement
- any per-column block voxelization
