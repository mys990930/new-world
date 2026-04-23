# hydrology

## Role

- own the post-smoothing hydrology stage that turns pre-defined drainage intent into final carved channels, basins, wet margins, and connected water surfaces
- consume already-shaped terrain rather than rediscovering branch topology from raw atlas fields

## Responsibilities

- consume `SmoothedPrototype` plus carried corridor constraints as the last terrain-shaping stage before voxelization
- solve connected river, lake, floodplain, wet-basin, and outlet water behavior from the already-owned branch model
- carve final hydrology-driven terrain shape such as channel beds, floodplain benches, spill paths, lake bowls, and wet lowland depressions into the post-smoothing surface
- preserve downstream continuity, basin outlets, coastal exits, and branch identity across chunk boundaries
- expose voxelization-facing per-column hydrology state so later material and block fill do not need to rediscover water placement
- decide where standing water, shallow saturation, or dry carved channels belong at launch scope

## Non-Responsibilities

- discovering long-range river topology from raw atlas fields alone
- replacing `RegionArchetype`, `HydrologyContext`, or atlas-owned drainage structure
- meso feature placement or smoothing ownership
- final material selection, block ids, or seasonal cover overrides
- vegetation, ecology, or weather simulation

## Inputs

- `ChunkCoord`
- `ChunkGenerationV2Inputs`
- `ChunkCorridorWindow`
- `SmoothedPrototype`

## Outputs

- `HydrologySolve`

## Current Interface

```rust
build_chunk_hydrology_solve(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationV2Inputs,
    corridor_window: &ChunkCorridorWindow,
    smoothed: &SmoothedPrototype,
) -> HydrologySolve
```

The current Rust implementation now exposes `build_chunk_hydrology_solve(...)` plus `empty_hydrology_solve(...)`.

## Current Types

- `HydrologyMode`
- `HydrologyColumn`
- `HydrologySolve`

### `HydrologyMode`

- coarse late-stage hydrology classification for one chunk column
- current cases:
  - `Dry`
  - `Channel`
  - `Floodplain`
  - `Lake`
  - `Wetland`

### `HydrologyColumn`

- one chunk-local post-hydrology column result
- currently stores:
  - final carved `terrain_height`
  - optional `water_surface_height`
  - optional `channel_floor_height`
  - `saturation`
  - `mode`

## Type Semantics

### `HydrologySolve`

- owns the full chunk-local post-smoothing hydrology result
- must be deterministic for the same `(seed, generator_version, chunk, inputs, corridor_window, smoothed)`
- should remain chunk-local in storage shape, but its waterline decisions must be derived from world-space branch continuity
- currently stores one `HydrologyColumn` per chunk column plus `connected_waterlines`

### Per-column hydrology result

- the target per-column solve should describe the final terrain surface handed to voxelization after hydrology carve
- if water is present, the solve should also describe the connected water surface for that column
- if a column is merely saturated or flood-prone without standing water, the solve should still preserve that distinction so material policy can react later
- hydrology carve may spend the remaining late-stage relief budget, but it should do so only where water structure justifies the terrain change
- every numeric field handed to downstream consumers must stay finite; non-finite intermediate corridor responses should be rejected or sanitized before they reach preview, voxelization, or tests

## Relationship To Earlier Stages

- `corridors` owns branch continuity, downstream intent, and broad valley-envelope guidance
- `prototype` owns the broad landform that makes rivers, basins, and outlets readable at long range
- `meso` adds local accents but should not erase primary corridor intent
- `smoothing` regularizes the pre-hydrology surface while preserving corridor and ridge intent
- `hydrology` is the first stage allowed to perform the final water-driven carve that commits the exact late terrain depression needed for channels, floodplains, basins, and shore-connected spill paths

## Relationship To Voxelization

- voxelization should consume hydrology output, not recalculate channels or lake depths from scratch
- hydrology must hand off enough information that voxelization can place water blocks, bed materials, banks, saturated soil, and dry exposed surfaces without a second terrain carve pass
- voxelization may quantize the carved result into actual block ids, but it must not invent a different downstream path or seal a hydrology-open outlet

## Processing Direction

1. read the smoothed post-meso surface for the target chunk
2. gather the relevant carried corridor branches, outlet context, and region hydrology modulation already assembled in `inputs`
3. solve connected longitudinal water surfaces and outlet anchors from branch identity plus downstream grade, not from isolated local puddle heuristics
4. derive local hydrology mode per column such as active channel, floodplain bench, lake bowl, wet basin margin, saturated lowland, or unaffected terrain
5. carve the final hydrology-driven terrain surface from the smoothed baseline while preserving major non-water landform identity outside the active water influence
6. assign standing-water or saturated-ground state from the connected solve
7. emit a chunk-local `HydrologySolve` for voxelization

## Current Minimal Implementation

- evaluates the nearest carried corridor response per column after smoothing
- blends biome, terrain-form, relief, and hydrology-context signals into late-stage river style knobs such as incision strength, transition softness, width variation, depth variation, meander support, outer spread, and confinement
- derives a late-stage carved terrain height plus optional visible standing water from corridor geometry, downstream anchor tendencies, wetness, basinness, and region hydrology context
- warps the carried branch into a deterministic meandered centerline before lateral distance falloff is evaluated, so the late carve can bend inside a macro corridor instead of staying locked to one straight segment
- uses world-space domain-warped noise to vary channel width, wetted width, bank asymmetry, bed depth, floodplain reach, and outer transition distance continuously along the branch
- composes the carve from nested outer-spread, floodplain, bank, and core incision layers instead of applying one flat y-delta inside one fixed mask, so the river influence can fade into the surrounding terrain rather than ending at a hard edge
- treats atlas corridor width as a broad valley-envelope hint, not as the final wetted width; visible water now appears only when the carried branch segment actually approaches the chunk
- only keeps standing water when the local carved trough and bank support can actually contain it; hydrology should not leave deep visible water perched on an uncarved shoulder
- includes a small basin fallback for standing-water bowls and wetland marking when no corridor dominates
- keeps the result deterministic and chunk-local while preserving shared-edge water continuity for neighboring chunks that see the same carried branch
- now normalizes late-stage saturation / height outputs to finite ranges before handoff so preview tinting and later consumers do not inherit `NaN` artifacts from rejected intermediate responses

## Carve Rules

- active channels
  - carve a bed that stays connected to the carried branch and respects downstream falloff
  - allow the final local carve to meander within the carried corridor envelope as long as endpoints and downstream continuity remain stable across chunk seams
  - derive wetted width, bank width, floodplain reach, outer influence reach, and bed depth separately so a wide macro corridor can still resolve to a narrower incised channel
  - vary width, depth, bank asymmetry, and direction continuously with world-space hydrology noise rather than keeping one uniform cross-section
  - widen and soften banks according to corridor role rather than forcing every branch into the same cross-section
- floodplains and wet lowlands
  - may lower or flatten terrain around active channels where region and corridor context support a broad wet opening
  - should remain shallower and wider than the main channel
  - may project a softer outer transition beyond the visible wetted ribbon so the carve can blend into adjacent terrain instead of terminating like a step cut
- lake basins and ponded outlets
  - may carve or preserve closed bowls only when the broader basin and outlet context supports standing water
  - should not trap water uphill from a valid basin spill path or coastal exit
- coastal and basin outlets
  - must stay open if earlier stages intentionally preserved them
  - hydrology may lower the outlet threshold, but it must not build a blocking lip

## Invariants

1. the same `(seed, generator_version, chunk)` must always produce the same connected hydrology result
2. neighboring chunks that see the same branch must agree on water-surface continuity and compatible carve depth at the shared edge
3. hydrology may lower or widen terrain where water structure requires it, but it must not replace unrelated ridge, plateau, or dune identity far from hydrology influence
4. connected water surfaces and channel floors must not step uphill along a single carried downstream branch
5. hydrology must remain downstream of smoothing and upstream of voxelization
6. final block materials and water voxels should derive from hydrology output plus surface policy, not from raw atlas wetness thresholds alone
7. `terrain_height`, `water_surface_height`, `channel_floor_height`, and `saturation` exposed through `HydrologyColumn` must always be finite when present

## Notes

- connected waterlines belong here, not in raw region classification
- final river, lake, floodplain, and wet-basin carve also belongs here, not in `corridors`, `prototype`, or `voxelize`
- a chunk that only sees a distant macro corridor envelope should stay carved-dry; hydrology should not paint standing water there just because prototype preserved a broad valley tendency
