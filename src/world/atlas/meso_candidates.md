# meso_candidates

## Role

- hold the current candidate list for meso terrain features
- record which candidates are true meso fits versus hybrid cases
- keep a deliberately small first-wave shortlist for implementation order

## Status

- the authoritative scaffolded per-feature pool now lives in `meso/catalog.md` and `meso/features/*`
- this file now acts as the grouping and wave-planning companion to that scaffold, not as the exact exhaustive index

## Candidate Pool

### Highland / Raised Landforms

- hill clusters
  - owner fit: meso
  - note: ideal first-wave candidate for plains and gentle uplands
- plateaus / local highlands
  - owner fit: meso
  - note: may overlap with macro elevation, but the readable local block should be meso
- local ridgelines / ridge spurs
  - owner fit: meso + structure
  - note: large mountain spines remain structure-owned
- summit groups / local peaks
  - owner fit: meso

### Lowland / Depressions

- valleys
  - owner fit: meso + structure
  - note: major drainage direction is structure-owned; local valley width and shape are meso
- basins
  - owner fit: meso
- canyons / gorges
  - owner fit: meso + structure
- deep gullies / ravines
  - owner fit: meso
- sinkholes
  - owner fit: meso

### Boundaries / Edge Forms

- escarpment bands
  - owner fit: meso
- coastal cliffs
  - owner fit: meso + coast context
- fault-like scarps
  - owner fit: meso
- softened cliff lines / stepped cliff zones
  - owner fit: meso
- terraces
  - owner fit: meso

### Water-Adjacent Terrain

- local lake basins
  - owner fit: meso + hydrology
- creek corridors
  - owner fit: meso + hydrology
- deltas
  - owner fit: meso + hydrology
- estuaries
  - owner fit: meso + hydrology + coast
- lagoons
  - owner fit: meso + hydrology + coast
- spits / barrier-like sediment forms
  - owner fit: meso + coast
- waterfalls
  - owner fit: special hybrid
  - note: needs river path plus strong local drop and later water presentation support

### Arid / Erosional

- dune fields
  - owner fit: meso
- badlands
  - owner fit: meso
- eroded coastal breakup
  - owner fit: meso + coast

### Cryo / Volcanic / Extreme Terrain

- glacier-carved local valleys
  - owner fit: meso + climate
- crevasse belts
  - owner fit: meso + climate
- ice shelf edge breakup
  - owner fit: meso + coast + climate
- lava fields
  - owner fit: meso
- craters / calderas
  - owner fit: meso
- wetlands / marsh flats
  - owner fit: meso + biome/cover

## Deferred Or Non-Meso Primary Cases

- savanna
  - primary owner: biome / cover
- steppe
  - primary owner: biome / cover
- grassland
  - primary owner: biome / cover
- tropical rainforest
  - primary owner: biome / cover
- conifer forest
  - primary owner: biome / cover
- snowfield
  - primary owner: biome / cover
- tundra
  - primary owner: biome / cover
- permafrost
  - primary owner: biome / cover with meso response
- natural arches
  - primary owner: special 3D feature system
- sea stacks
  - primary owner: special 3D or placed feature system
- icebergs
  - primary owner: detached/special feature system
- reefs
  - primary owner: coast ecology / shallow-water system

## First-Wave Shortlist

The first wave should favor terrain that:

- is clearly readable inside a `2..6 chunk` play view
- does not require a new 3D feature system
- does not depend on a fully reworked river or biome stack
- can be expressed as smooth heightfield deformation plus later material bias

### Wave 1A

- hill clusters
  - status: implemented
  - why: immediately improves plains and gentle inland terrain readability
- basins
  - status: implemented
  - why: gives lowland identity and later supports lakes/wetlands naturally
- escarpment bands
  - status: implemented
  - why: gives strong silhouette and readable terrain transition without needing full canyon logic
- terraces
  - status: implemented
  - why: useful across uplands, coasts, and basin shoulders with relatively simple heightfield logic

### Wave 1B

- canyons / gorges
  - why later: stronger river and drainage coupling
- coastal cliffs
  - why later: wants coast-aware meso plus shoreline material handling
- dune fields
  - why later: wants better aridity/material coupling
- craters
  - why later: still good meso, but less foundational than hills/basins/escarpments

## Planned Selection Rule For Wave 1

- hill clusters
  - favored by inland plains, low-to-mid ruggedness, non-river corridors
- basins
  - favored by inland low relief, wetter zones, weak ridge influence, weak coast influence
- escarpment bands
  - favored by upland transitions, stronger ruggedness, ridge shoulders, and some coast-adjacent zones
- terraces
  - favored by upland or coastal slopes where macro relief exists but should step rather than spike

## Design Preference

- candidate choice should be guided by atlas and structure context first
- candidate choice should be further constrained by resolved region archetype before local randomness is allowed
- within that allowed set, deterministic randomness should choose whether a place becomes `hill cluster` versus `basin` versus `escarpment`
- parameter variation such as width, elongation, depth, sharpness, and heading should also be randomized inside context-dependent bounds

## Current Implementation Notes

- the current implementation emits blended guide channels rather than a hard feature label map
- hills and basins currently accumulate as broad additive/depressive relief hints
- escarpments and terraces currently emit dominant signed-distance plus heading hints so generation can apply step-like deformation without introducing per-block noise
- Wave 1B remains deferred until river/coast coupling is further expanded
