# status

## Role

- summarize the current world-generation transition after legacy V1 removal
- show which modules are authoritative for V2 planning, which public APIs are now compile-only stubs, and what still remains to implement

## Current Direction

The project now treats region-first V2 generation as the only intended terrain architecture.

The old V1 realization stack has been removed instead of kept alive in parallel. Public generation entrypoints still exist so the rest of the workspace can compile, but they are now explicit TODO stubs until V2 chunk realization is implemented.

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
- `src/world/generation/v2/*`
  - V2 stage scaffolds for inputs, corridors, prototype, meso apply, smoothing, hydrology, and voxelization
- `src/world/generation/sampler.rs`
  - shared helper that gathers the padded atlas / structure / region / meso windows for a target chunk
  - current implementation now snaps neighboring chunks to a shared structure-sized atlas-context tile before padding and assembles raw atlas scalar cells from canonical region-owned field solves so the same `AtlasCoord` stays stable across chunk requests

### Kept As Temporary Compile Surface

- `src/world/generation/mod.rs`
  - public generation facade
  - `generate_chunk(...)` is now an explicit TODO stub
  - probe helpers are also TODO stubs
- `src/world/generation/profile.rs`
  - retains only the diagnostic `TerrainProfile` enum used by tools and previews
- `src/world/generation/probe.rs`
  - retains only public probe / LOD data types so callers still compile

### Removed Legacy V1 Implementation

- `context.rs`
- `noise.rs`
- `realize.rs`
- `surface.rs`
- `legacy.rs`
- `profiles/*`

These files previously owned the actual V1 chunk realization pipeline, terrain-profile math, material fill, hydrology carve, and debug probing. They are intentionally gone now rather than being kept beside V2.

## V2 Pipeline Progress

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

### 4. River Corridor Solve

- status: `implemented as initial standalone solve`
- owner: `v2/corridors.rs`
- note:
  - `build_chunk_corridor_window(...)` now converts nearby drainage segments plus region / field context into deterministic chunk-local corridor constraints
  - corridor centers may sit outside the strict chunk bounds when broad neighboring reaches still influence prototype shaping
  - the current solver now carries actual corridor segment geometry, keeps every geometry-overlapping influencer instead of truncating to a tiny top-N subset, and derives width / grade from intrinsic segment context so neighboring chunks agree on shared branches
  - the stage contract remains the same: corridor solve consumes structure + region context and emits prototype-facing branch constraints before base heightfield solving
  - scaffold assembly now carries corridor output forward, and the prototype boundary accepts it explicitly

### 5. Biome-Aware Base Heightfield

- status: `implemented as initial broad-shape solve`
- owner: `v2/prototype.rs`
- note:
  - `src/world/generation/v2/prototype.md` is now the authoritative base-heightfield solve design
  - prototype now emits one `PrototypeColumn` per in-chunk column, using region/archetype context plus corridor constraints to produce a deterministic broad landform scaffold
  - current implementation now samples atlas scalars continuously in block/world space, blends neighboring classified region context near atlas boundaries without chunk-wide family snapping, uses canonical atlas field assembly to avoid chunk-request drift, collapses repeated river-branch segment responses so long corridors do not stack into seam walls, and adds deterministic subchunk ripple/terrace variation so low-relief terrain reads more clearly in quarter-view
  - corridor response, valley seats, and relief budgets remain explicit, while shared-edge regression tests now guard against chunk, atlas-boundary, and canonical-region boundary step artifacts
  - later stages still own meso accents, smoothing, final hydrology, and voxel/material realization

### 6. Meso Solve

- status: `implemented as initial Wave 1 chunk deformation pass`
- owner:
  - atlas ownership: `atlas/meso.rs`
  - chunk-side application scaffold: `v2/meso_apply.rs`
- note:
  - guide ownership and a full per-feature scaffolded catalog exist
  - the scaffolded pool now carries `launch / extended / deferred` labels and per-feature planning stubs
  - runtime guide generation still only emits the current Wave 1A subset from `atlas/meso.rs`
  - some landform-owned launch archetypes intentionally keep launch meso empty or nearly empty until prototype solving exists, notably `desert_dune_field` and `glaciated_alpine`
  - the current chunk-side apply stage now samples those guides per block column after prototype and before smoothing
  - runtime gating is currently conservative and temporary: the stage only applies the Wave 1 core subset and consults each archetype's current `allowed_meso_keys` stub until the authoritative per-archetype matrix is published
  - avoid-primary-corridor behavior is enforced in the chunk-side pass so meso does not overwrite broad river corridor intent
  - authoritative per-archetype allowance matrix still needs to be locked and may tighten the current temporary gate

### 7. Local Detail / Smoothing

- status: `scaffold only`
- owner: `v2/smoothing.rs`

### 8. Final Hydrology

- status: `scaffold only`
- owner: `v2/hydrology.rs`

### 9. Region / Material Policy

- status: `definitions exist, not wired into generation`
- owner: `src/world/surface/material.rs`, `src/world/surface/cover.rs`

### 10. Seasonal Biome State

- status: `definitions exist, not wired into generation`
- owner: `src/world/surface/seasonal.rs`

### 11. Voxelization

- status: `scaffold only`
- owner: `v2/voxelize.rs`

## What Was Intentionally Broken

The following public calls now compile but no longer function at runtime:

- `generate_chunk(...)`
- `probe_chunk(...)`
- `probe_column(...)`
- `sample_chunk_surface_lod(...)`

This is intentional. We are no longer pretending the removed V1 generator is still an acceptable fallback.

## What Still Remains Before V2 Can Replace It

1. publish the authoritative archetype-to-meso allowance matrix for the locked launch set
2. lock launch material policy per archetype
3. lock launch seasonal biome-state policy
4. document launch fallback behavior for extended and deferred archetypes
5. expand and tune archetype coverage in `v2/prototype.rs` as more launch and extended landform cases come online
6. tune and tighten the Wave 1 meso operators in `v2/meso_apply.rs` once the authoritative matrix is published
7. implement smoothing/local refinement in `v2/smoothing.rs`
8. implement connected hydrology in `v2/hydrology.rs`
9. implement final material + block voxelization in `v2/voxelize.rs`
10. replace the compile-only generation stubs with real V2 behavior

## Short Summary

We now have:

- atlas climate/geography inputs
- atlas skeleton
- region classification scaffolding
- meso catalog scaffolding
- surface policy scaffolding
- V2 stage containers

We no longer have:

- a functioning fallback terrain generator

That tradeoff is deliberate so the codebase stops drifting around V1 assumptions and forces the next work directly through the V2 pipeline.
