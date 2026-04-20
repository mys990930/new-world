# prototype

## Role

- own the biome-aware base heightfield solve before meso deformation
- turn region archetype plus corridor constraints into the broad landform that later stages refine
- remain the authoritative design contract for the base-heightfield solve stage

## Responsibilities

- convert `RegionArchetype`, atlas scalar fields, nearby mountain structure, and `ChunkCorridorWindow` into a deterministic broad-shape prototype
- sample atlas scalar inputs continuously in world/block space with smoothed fractional interpolation instead of stepping the whole chunk on atlas-cell boundaries
- blend local region-family context near atlas boundaries by first blending shared basis parameters rather than directly averaging unrelated per-family outputs
- derive ridge, coast, basin, and corridor influence as continuous world-space basis terms that can cross chunk borders naturally
- preserve valley seats, floodplain openings, basin outlets, and coastal exits already established by corridor solve
- soft-blend repeated corridor segment responses per river branch so adjacent segments do not stack into artificial trench walls or hand off with sharp seams
- inject deterministic subchunk relief bands and ripple-sized height changes from seed-independent value-noise carriers so low-relief terrain still reads clearly in the quarter-view camera
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

## Coordinate Space

- columns are chunk-relative and aligned to the chunk's block-grid footprint
- `base_height` is expressed in world-space block Y, not in atlas-cell units
- `relief_budget` is a nonnegative budget in block units describing how much local up/down shape can still be introduced later without erasing the broad form
- corridor segment endpoints and centers may lie outside the strict chunk bounds, but the influence on prototype columns is still evaluated in chunk-local block space
- atlas scalar sampling is evaluated from block-center world coordinates through a smoothed fractional atlas lookup, while region identity is blended from the neighboring classified atlas cells around the same sample point
- nearby mountain-chain segments are sampled into chunk-local ridge basis signals from world-space distance and shoulder falloff rather than by hard atlas-cell ownership

## Prototype Semantics

- `base_height`
  - the provisional solid-surface target for the column before meso and later detail
  - not the final exposed surface after smoothing, hydrology, or voxelization
  - may represent a plateau top, valley floor, shelf break, ridge shoulder, basin bowl, dune crest neighborhood, or coastal rim depending on family
- `relief_budget`
  - how much local vertical texture the later pipeline may still spend in the column
  - should shrink when the broad landform is already sharp, exposed, or corridor-sensitive
  - should stay larger in plains, terraces, and other areas that still need room for meso and smoothing to do visible work

## Solve Model

- the prototype now treats height as a shared continuous basis solve, not as a per-family standalone formula
- each sampled region family contributes a weighted parameter set describing:
  - macro uplift bias
  - coastal shelf / apron / cliff response
  - ridge crest / shoulder lift
  - basin lowering
  - inland / aridity / wetness bias
  - low-frequency, structure-aligned, terrace, and dune amplitudes
  - corridor depth / width / outlet-open behavior
- the prototype first blends those parameters from neighboring region samples around the current world-space point
- the final broad height is then evaluated once from common basis terms such as macro elevation, coastal response, ridge structure response, basin response, seed-independent deterministic detail, and corridor response
- deterministic detail carriers should stay stable across world seeds, while region, atlas, and corridor context only modulate where and how strongly those carriers are expressed
- this keeps boundaries readable while avoiding abrupt "switch formula" behavior at atlas-cell or classified-region edges

## Launch Policy Families

- the prototype solve should group launch archetypes into broad shape families instead of handling each archetype as a unique algorithm
- families should be stable enough that later extended archetypes can slot into the same parameter-set structure without rethinking the solve boundary

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
2. blend nearby region-family context into a shared parameter set so atlas-cell boundaries transition gradually instead of introducing abrupt base-height steps
3. sample nearby mountain-chain structure into continuous ridge-core and ridge-shoulder basis signals in world space
4. read the corridor window and convert each branch into continuous valley, floodplain, and outlet-openness signals with chunk-external support
5. evaluate the shared basis solve once from macro elevation, coast, ridge, basin, and deterministic detail terms
6. softly blend repeated river-path segment responses by branch so a long river does not over-carve or abruptly hand off where adjacent segments overlap the same column
7. apply corridor response as a pre-meso constraint on top of the shared basis solve while preserving ridge shoulders where appropriate
8. distribute relief budget to preserve room for later meso and smoothing without changing the broad identity
9. emit one `PrototypeColumn` per chunk column and return the completed `BaseHeightfieldPrototype`

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

## Boundary And Continuity Rules

- region boundaries should blend shared solver parameters, not switch to a different closed-form height equation
- ridge and corridor influence should be evaluated in world space from nearby segment geometry, even when the segment midpoint lies outside the strict chunk footprint
- atlas scalar interpolation should use smoothed fractions so the same cell neighborhood does not create a visible terrace merely because the sample crossed an atlas-cell line
- any deterministic micro-relief kept in prototype should remain subordinate to the broad basis terms, should never depend on chunk generation order, and should keep its carrier pattern stable across world seeds

## Invariants

1. deterministic for the same inputs and world seed
2. seam continuity across chunk boundaries is mandatory
3. corridor continuity is mandatory
4. atlas-cell boundaries must not introduce artificial base-height steps just because the chunk crossed into a new input window
5. only broad-shape information belongs here
6. final hydrology is deferred
7. material and block decisions are deferred
8. meso accents are deferred
9. local smoothing detail is deferred, but prototype may still carry deterministic subchunk banding when that is needed to keep terrain legible at gameplay camera scale

## Deferred

- final river-bed depth and water surface solve
- sediment, topsoil, stone, and block material choice
- small-scale contour breakup
- weathering or seasonal surface state
- vegetation cover or ecology placement
- any per-column block voxelization
