# coastal_cliff_band

## Stage

- `launch`

## Identity

- placement family: `CoastalEdge`
- hydrology coupling: `RequiresCoast`

## Summary

- Deterministic multi-chunk coastal cliff bands resolved as world-space cliff objects instead of a per-column shoreline guess.

## Runtime Ownership

- Atlas guide emission remains broad and shared.
- Feature-owned runtime resolve lives entirely under `features/coastal_cliff_band/`.
- Runtime ownership is `MesoRegion`-based so neighboring chunks overlapping the same coastal cliff footprint sample the same resolved object set.
- The launch runtime builds a `CoastalCliffBandWindow` first, then samples that window per column.

## Current Runtime Input Contract

- Until shared meso dispatch emits a dedicated coastal-cliff guide family, this feature reads the existing `escarpment_*` guide channels as its temporary runtime input contract.
- `escarpment_heading_*` defines the coast-parallel cliff heading.
- `escarpment_signed_distance_cells` is interpreted as a sampled offset from the band centerline and is used to reconstruct a world-space cliff line.
- The current feature-local runtime assumes the positive guide normal is the landward side of the cliff object.
- Main-thread integration may later add explicit coast-side orientation or a filtered guide map without changing the feature-owned window/sample shape contract.

## Terrain Traits

- Breaks up shoreline or coast-parallel terrain without replacing the owning coastal archetype.
- Resolves a cliff crest, landward shelf, and outer bench as separate shape signals so the face is readable without forcing one shared additive `delta_y`.
- Keeps the strongest uplift on the assumed landward side and only a lighter bench lift on the seaward side.
- Should stay continuous across chunk and meso-cell boundaries because the same world-space cliff objects are reused from the resolved window.

## Feature-Owned Helpers

- `build_coastal_cliff_band_window(...)`
- `sample_coastal_cliff_band_apply_signal_from_window(...)`
- `sample_coastal_cliff_band_surface_from_window(...)`
- `debug_coastal_cliff_band_peak_candidates(...)`
- `debug_coastal_cliff_band_resolved_objects_from_window(...)`

## Surface Contract

- `CoastalCliffBandApplySample` keeps feature-local shape signals:
  - face coverage
  - plateau coverage
  - bench coverage
  - crest raise hint
  - bench lift hint
  - signed-distance debug read
- `CoastalCliffBandSurfaceSample` returns:
  - `target_surface_y`
  - `blend_weight`
  - `relief_spend`
  - feature-local coverage masks for later debugging or orchestration

## Test Focus

- deterministic resolved-window output for the same guides
- stable resolved ownership across neighboring chunk windows
- no visible seam across a shared chunk boundary
- flat output when cliff guides are absent
- landward plateau stays higher than the seaward bench
- strong cliffs keep materially visible uplift

## Ecology Notes

- Later ecology can separate exposed cliff or spray-tolerant cover from inland cover.
- Later material policy can map crest and face coverage to rockier exposed surfaces than the shelf behind the cliff.

## Follow-up

- shared meso dispatch still needs to wire `coastal_cliff_band` into runtime orchestration outside this folder
- main-thread integration may later pass explicit coast-side orientation or isolate coastal-cliff-only guide maps before sampling
