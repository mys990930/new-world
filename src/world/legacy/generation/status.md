# status

## Role

- summarize the active world-generation pipeline
- show which modules are authoritative for current generation planning, which public APIs are now compile-only stubs, and what still remains to implement

## Current Direction

The project now treats region-first generation as the only intended terrain architecture.

The main `generate_chunk(...)` path runs the initial end-to-end region-first generation stack, while the public probe helpers still remain compile-only TODO stubs until richer diagnostics land.

## Current Module Map

### Kept As Authoritative

- `src/world/atlas/atlas_fields.rs`
  - atlas-scale raw climate and geography inputs
- `src/world/atlas/structure.rs`
  - macro mountain / drainage skeleton
- `src/world/atlas/region.rs`
  - deterministic region classification dimensions and `RegionArchetype` resolution
- `src/world/atlas/region/archetypes/*`
  - per-archetype planning stubs with launch / extended / deferred labels
- `src/world/atlas/meso.rs`
  - atlas-owned meso guide ownership and catalog hookup
- `src/world/atlas/meso/features/*`
  - per-feature planning stubs
- `src/world/surface/*`
  - material, cover, and seasonal surface policy scaffolds
- `src/world/generation/*`
  - generation stage scaffolds for inputs, realization field, corridors, prototype, meso apply, smoothing, hydrology, and voxelization
- `src/world/generation/sampler.rs`
  - shared helper that gathers the padded atlas / structure / region / meso windows for a target chunk
  - current implementation now snaps neighboring chunks to a shared structure-sized atlas-context tile before padding and assembles raw atlas scalar cells from canonical region-owned field solves so the same `AtlasCoord` stays stable across chunk requests

### Kept As Temporary Compile Surface

- `src/world/generation/mod.rs`
  - public generation facade
  - `generate_chunk(...)` now runs the initial generation terrain-to-block path
  - probe helpers are still TODO stubs
- `src/world/generation/profile.rs`
  - retains only the diagnostic `TerrainProfile` enum used by tools and previews
- `src/world/generation/probe.rs`
  - retains only public probe / LOD data types so callers still compile

### Removed Legacy Implementation

- `context.rs`
- `noise.rs`
- `realize.rs`
- `surface.rs`
- `legacy.rs`
- `profiles/*`

These files previously owned an older chunk realization pipeline, terrain-profile math, material fill, hydrology carve, and debug probing. They are intentionally gone rather than being kept beside the active generation path.

## Generation Pipeline Progress

### 1. Atlas Raw Fields

- status: `implemented`
- owner: `atlas_fields.rs`
- note:
  - continuous climate / geography signals already exist and remain authoritative

### 2. Atlas Skeleton

- status: `implemented`
- owner: `structure.rs`
- note:
  - macro mountain-chain and drainage graph scaffolds already exist

### 3. Region Classification

- status: `implemented as initial classifier + scaffolded catalog`
- owner: `region.rs`, `region/catalog.rs`, `region/archetypes/*`
- note:
  - dimensions, family taxonomies, and the scaffolded `RegionArchetype` candidate pool are in place
  - `src/world/atlas/region_catalog.md` is now the authoritative launch design contract for that candidate pool
  - launch policy now allows strong hydrology context to override otherwise generic inland archetype resolution
  - archetype-to-meso, seasonal, and material policy still need locking

### 4. Realization Field

- status: `implemented as initial continuous control-field solve`
- owner: `realization_field.rs`
- note:
  - this stage now sits between discrete region classification and prototype height solving
  - it converts atlas-cell semantic classes plus nearby atlas context into a continuous prototype-control field, so prototype no longer reads classified atlas cells as its first parameter lattice
  - the current implementation uses a `2 x 2` chunk realization lattice with halo support, source hints derived from displaced region-influence semantics plus atlas scalars, and a local permeability-weighted diffusion solve before world-space sampling
  - the stage now also emits broad material support axes for wetness, exposure, sediment, and stable soil/cover so final surface material boundaries can follow connected terrain evidence instead of hard atlas owner edges
  - the stage is intentionally local and chunk-order-independent, but still initial: tuning, channel coverage, and stronger macro-barrier rules remain open work

### 5. River Corridor Solve

- status: `implemented as initial standalone solve`
- owner: `corridors.rs`
- note:
  - `build_chunk_corridor_window(...)` now converts nearby drainage segments plus region / field context into deterministic chunk-local corridor constraints
  - corridor centers may sit outside the strict chunk bounds when broad neighboring reaches still influence prototype shaping
  - the current solver now carries actual corridor segment geometry, keeps every geometry-overlapping influencer instead of truncating to a tiny top-N subset, and derives width / grade from intrinsic segment context so neighboring chunks agree on shared branches
  - the stage contract remains the same: corridor solve consumes structure + region context and emits prototype-facing branch constraints before base heightfield solving
  - scaffold assembly now carries corridor output forward, and the prototype boundary accepts it explicitly

### 6. Biome-Aware Base Heightfield

- status: `implemented as initial broad-shape solve`
- owner: `prototype.rs`
- note:
  - `src/world/generation/prototype.md` is now the authoritative base-heightfield solve design
  - prototype now emits one `PrototypeColumn` per in-chunk column, using realization-field control samples plus corridor constraints to produce a deterministic broad landform scaffold
  - current implementation now samples atlas scalars continuously in block/world space, blends neighboring classified region context near atlas boundaries without chunk-wide family snapping, uses canonical atlas field assembly to avoid chunk-request drift, collapses repeated river-branch segment responses so long corridors do not stack into seam walls, and adds deterministic subchunk ripple/terrace variation so low-relief terrain reads more clearly in quarter-view
  - direct classified-cell blending is no longer the primary control path; broad-shape parameters and corridor-mode policy weights now both come from realization-era continuous signals plus atlas/structure context
  - corridor response, valley seats, and relief budgets remain explicit, while shared-edge regression tests now guard against chunk, atlas-boundary, and canonical-region boundary step artifacts
  - later stages still own meso accents, smoothing, final hydrology, and voxel/material realization

### 7. Meso Solve

- status: `implemented as initial launch chunk surface-resolution pass`
- owner:
  - atlas ownership: `atlas/meso.rs`
  - chunk-side application scaffold: `meso_apply.rs`
- note:
  - guide ownership and a full per-feature scaffolded catalog exist
  - the scaffolded pool now carries `launch / extended / deferred` labels and per-feature planning stubs
  - runtime guide generation in `atlas/meso.rs` still emits only the broad Wave 1A channel set
  - shared meso lottery and dispatch stay in `atlas/meso.rs`, but runtime-wired feature-specific hill-cluster shaping now lives under `atlas/meso/features/hill_cluster/`
  - some landform-owned launch archetypes intentionally keep launch meso empty or nearly empty until prototype solving exists, notably `desert_dune_field` and `glaciated_alpine`
  - the current chunk-side apply stage now samples those guides per block column after prototype and before smoothing
  - runtime gating is currently conservative and temporary: the stage consults each archetype's current `allowed_meso_keys` stub until the authoritative per-archetype matrix is published
  - the target architecture is now feature-owned meso surface resolution: `meso_apply.rs` should orchestrate gating, corridor policy, compositing, and relief accounting, while each feature module owns its own target local surface logic
  - the current runtime-backed launch subset is:
    - `hill_cluster`
    - `shallow_basin`
    - `escarpment_band`
    - `upland_terrace`
    - `ravine`
    - `coastal_cliff_band`
    - `dune_field`
    - `crater`
  - hill clusters now use atlas-owned broad hill guides plus feature-owned sparse independent-hill resolution in the chunk pass with dominant-peak ownership pruning, low-amplitude support shoulders, source-stable fallback orientation, stronger visible uplift tuning, basin-aware compositing, summit-dominant uplift weighting, and a shared resolved hill-object window assembled from stable meso-region-owned hills instead of per-column hill re-interpretation
  - `ravine`, `coastal_cliff_band`, `dune_field`, and `crater` now also use feature-owned resolved windows in the chunk pass, but they still borrow existing Wave 1A guide channels provisionally until dedicated atlas-side channels are published
  - ravine runtime resolve now treats ravines as sparse medium-size drainage valleys with stricter source thresholds, wider source spacing, downstream width/depth progression, sharper trench cuts, broader uneven shoulders, and angular centerline kinks instead of repeated short straight or smooth S-curve capsule cuts
  - per-column archetype allowance now reuses the same displaced region-influence sampling as realization/material transitions, so meso gates no longer fall back to a separate atlas-aligned semantic boundary
  - broad launch fallback targets now withhold `ravine` by default so temporary archetype fallback does not make ordinary plains, saturated lowlands, rainforest lowlands, savanna plains, desert plains, or generic glaciated alpine regions overproduce ravines
  - meso feature docs now distinguish the shared `2 x 2` chunk guide lattice from feature-owned small / medium / large resolved footprints so different meso families are not implicitly forced into the same visual window size
  - avoid-primary-corridor behavior is enforced in the chunk-side pass so meso does not overwrite broad river corridor intent
  - authoritative per-archetype allowance matrix still needs to be locked and may tighten the current temporary gate

### 8. Local Detail / Smoothing

- status: `implemented as initial constrained local refinement pass`
- owner: `smoothing.rs`
- note:
  - `build_chunk_smoothed_prototype(...)` now performs a minimal post-meso in-chunk smoothing pass
  - the current implementation only spends a small portion of remaining relief budget, suppresses blending near carried corridors and sharp local landforms, and leaves chunk-border columns unchanged to avoid introducing new seam drift
  - the stage also derives local slope and signed concavity hints for later hydrology / voxelization work

### 9. Final Hydrology

- status: `implemented as initial connected carve and water solve`
- owner: `hydrology.rs`
- note:
  - `build_chunk_hydrology_solve(...)` now consumes the post-smoothing surface plus carried corridor intent
  - the current implementation emits one hydrology result per chunk column with carved terrain height, optional water surface, optional floor depth, saturation, and coarse hydrology mode
  - region hydrology style sampling now reuses the same displaced region-influence neighborhood as realization/material transitions, so late water style no longer reintroduces a separate atlas-aligned boundary basis
  - final connected water solve and hydrology-driven terrain carve both belong here, and `chunk_preview --stage hydrology` now renders that stage directly
  - voxelization should read its output rather than recarving channels later

### 10. Region / Material Policy

- status: `implemented as initial chunk surface-plan resolve`
- owner: `src/world/surface/material.rs`, `src/world/surface/domain.rs`, `src/world/surface/cover.rs`, `src/world/surface/resolve.rs`
- note:
  - launch-oriented material policies now carry actual block palette keys instead of hint-only ids
  - material-domain selection sits between sampled region ownership and final block stacks so coherent visible material provinces can move away from raw atlas-cell edges without per-column speckle
  - surface resolve now consumes generation-derived material support axes from the smoothed terrain, so unsupported wetland/alpine/coastal defaults degrade or transition based on local evidence rather than exposing atlas-cell rectangles
  - `resolve_chunk_surface_plan(...)` now maps sampled region ownership, material-domain selection, hydrology, and local slope hints into one column plan with quantized terrain/water tops and top/filler/core block keys
  - hydrology may override local sediment expression, but it does not replace region ownership

### 11. Seasonal Biome State

- status: `implemented as initial optional runtime layer`
- owner: `src/world/surface/seasonal.rs`
- note:
  - the generation-side surface resolver can now attach a seasonal biome state when an explicit runtime context is provided
  - the baseline `generate_chunk(...)` path still keeps that state optional until world-owned calendar/runtime climate is threaded into generation

### 12. Voxelization

- status: `implemented as initial block fill`
- owner: `voxelize.rs`
- note:
  - `build_chunk_voxelization_plan(...)` now converts the resolved chunk surface plan into a per-column block-write plan
  - `voxelize_chunk(...)` writes top, filler, core, and standing-water blocks into `ChunkData`
  - the public `generate_chunk(...)` path now uses that initial end-to-end current generation flow instead of staying a TODO stub

## What Was Intentionally Broken

The following public calls now compile but no longer function at runtime:

- `probe_chunk(...)`
- `probe_column(...)`
- `sample_chunk_surface_lod(...)`

This is intentional. Removed generation paths are not acceptable fallbacks for active terrain output.

## What Still Remains

1. publish the authoritative archetype-to-meso allowance matrix for the locked launch set
2. lock launch material policy per archetype
3. lock launch seasonal biome-state policy
4. document launch fallback behavior for extended and deferred archetypes
5. tune and extend the generation-side realization-field stage so atlas-cell semantic classes stop projecting directly into large-scale terrain rectangles at larger scales too
6. expand and tune archetype coverage in `prototype.rs` and `realization_field.rs` as more launch and extended landform cases come online
7. publish dedicated atlas-side guide channels for runtime-backed features that currently borrow the Wave 1A channel set, especially `ravine`, `coastal_cliff_band`, `dune_field`, and `crater`
8. tune and extend smoothing/local refinement in `smoothing.rs`
9. tune and extend the initial connected hydrology and hydrology-driven terrain carve in `hydrology.rs`
10. tune and extend material-domain selection, surface-plan palette mapping, and hydrology material overrides beyond the locked launch set
11. thread authoritative world-owned season/runtime climate context into chunk surface resolve
12. tune and extend the initial voxel block fill in `voxelize.rs`, including better quantization and deeper per-policy layering
13. replace the remaining compile-only probe stubs with real generation diagnostics
14. enforce the generation artifact-suppression contract end to end: atlas-cell material rectangles, guide-cell squares, raw structure segment lines, repeated smooth corridor arcs, and uniform meso bands must be treated as regressions and covered by preview-oriented diagnostics

## Short Summary

We now have:

- atlas climate/geography inputs
- atlas skeleton
- region classification scaffolding
- meso catalog scaffolding
- surface policy scaffolding
- generation stage containers

We no longer have:

- a hidden fallback terrain path

That tradeoff is deliberate so the codebase stops drifting around removed terrain assumptions and keeps the remaining work focused on tuning and extending the active generation path that now reaches real block output.
