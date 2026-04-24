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

## Internal Submodules

- `hydrology.rs`
  - public hydrology types and chunk-level orchestration
  - selects the strongest corridor response per column, applies basin fallback, sanitizes output, and counts connected waterlines
- `hydrology/context.rs`
  - internal DTOs shared across hydrology passes, including branch keys, region style signals, corridor responses, and projected segment points
- `hydrology/water_profile.rs`
  - resolves the local longitudinal water-profile anchor before terrain shaping
  - produces the common water-surface baseline and core floor target used by downstream passes
- `hydrology/channel_carve.rs`
  - computes active-channel incision layers and the base carved terrain height
  - keeps core, bank, floodplain, and outer cut deltas explicit for later deposition/bench passes
- `hydrology/floodplain_bench.rs`
  - computes channel-adjacent shelf and floodplain bench adjustments that soften the carved transition
  - returns inside-bend alignment for later bar/point-bar decisions
- `hydrology/bars.rs`
  - owns gravel bar / point-bar strength and near-water bar terrain adjustment
  - creates explicit bank-side depositional flats against the expected visible-water reference rather than only painting a material mask
  - keeps same-height depositional bands continuous across weak-curvature reaches, while allowing stronger bends to shift or concentrate the bar side
- `hydrology/water_surface.rs`
  - decides whether the final shaped column can hold visible standing water
  - remains downstream of terrain shaping so water does not appear on unsupported shoulders
- `hydrology/masks.rs`
  - derives basin, saturation, water-presence, and final hydrology-mode masks for voxel/material policy

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
  - `gravel_bar_strength`
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
3. resolve shared branch context and region hydrology style signals
4. solve water-profile anchors from branch identity plus downstream grade, not from isolated local puddle heuristics
5. carve active channel layers against the shared profile
6. apply floodplain / bank bench shaping to blend the carve into surrounding terrain
7. apply gravel bar / point-bar shaping outside the wetted ribbon where deposition is likely
8. decide visible water surface only after terrain can physically support it
9. derive hydrology masks and final mode per column such as active channel, floodplain bench, lake bowl, wet basin margin, saturated lowland, or unaffected terrain
10. emit a chunk-local `HydrologySolve` for voxelization

## Current Minimal Implementation

- evaluates the nearest carried corridor response per column after smoothing
- now splits the monolithic corridor solve into internal pass modules for context, water profile, channel carve, floodplain bench, bars, visible water surface, and hydrology masks while preserving the public `build_chunk_hydrology_solve(...)` contract
- blends biome, terrain-form, relief, and hydrology-context signals into late-stage river style knobs such as incision strength, transition softness, width variation, depth variation, meander support, outer spread, and confinement
- derives a late-stage carved terrain height plus optional visible standing water from corridor geometry, downstream anchor tendencies, wetness, basinness, and region hydrology context
- warps the carried branch into a deterministic meandered centerline before lateral distance falloff is evaluated, so the late carve can bend inside a macro corridor instead of staying locked to one straight segment
- uses world-space domain-warped noise to vary channel width, wetted width, bank asymmetry, bed depth, floodplain reach, and outer transition distance continuously along the branch
- composes the carve from nested outer-spread, floodplain, bank, and core incision layers plus noisy bank-shelf / bench lifting instead of applying one flat y-delta inside one fixed mask, so the river influence can fade into the surrounding terrain rather than ending at a hard edge
- adds extra world-space carve breakup so channel-adjacent cut depth, bench height, and floodplain transitions do not resolve into one perfectly uniform band
- treats atlas corridor width as a broad valley-envelope hint, not as the final wetted width; visible water now appears only when the carried branch segment actually approaches the chunk
- only keeps standing water when the local carved trough and bank support can actually contain it; hydrology should not leave deep visible water perched on an uncarved shoulder
- uses corridor ownership to detect local parent-child confluences and raises deposition strength near inside bends, widenings, slow reaches, and junctions
- now emits `gravel_bar_strength` from an actual near-water bench pass so later voxel/material policy can recognize adjacent depositional benches without rediscovering them from scratch
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
- gravel bars and depositional benches
  - flatten terrain immediately beside water toward a near-water freeboard where inside bends, local widenings, parent-child confluences, or slowing reaches imply lower transport energy
  - should stay outside the core wetted ribbon and read as a bank-adjacent gravel flat before the valley wall or hillside climbs away, not as a mid-channel blockage or generic rocky exposure
  - should be close enough to the local water surface to read as a depositional bar / creekside gravel beach rather than a high terrace
  - should continue along matching depositional height bands unless branch curvature changes enough to move deposition to the opposite bank or concentrate it into a point bar
  - should remain deterministic and continuous with the owning branch across chunk seams
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
8. `gravel_bar_strength` exposed through `HydrologyColumn` must always stay finite and normalized to `0..1`

## Notes

- connected waterlines belong here, not in raw region classification
- final river, lake, floodplain, and wet-basin carve also belongs here, not in `corridors`, `prototype`, or `voxelize`
- a chunk that only sees a distant macro corridor envelope should stay carved-dry; hydrology should not paint standing water there just because prototype preserved a broad valley tendency
