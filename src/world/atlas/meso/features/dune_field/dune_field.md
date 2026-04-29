# dune_field

## Stage

- `launch`

## Identity

- placement family: `AridExposure`
- hydrology coupling: `PrefersAridRunoff`

## Summary

- Feature-owned runtime resolver for deterministic multi-ridge dune groups under dry regional conditions.

## Terrain Traits

- Atlas/shared meso ownership should still emit only broad dune suitability, dominant crest spacing, and dominant crest heading rather than a final per-column dune waveform.
- Runtime resolve should convert that broad guide support into a smaller set of world-space dune-field objects owned by a stable meso-region layer instead of re-guessing dunes per chunk or per sampled column.
- A resolved dune field should read as several parallel or gently offset dunes with visible interdune spacing, not as one smooth mesa-like uplift.
- Broad dune-field support should stay separate from sharper crest height so dry transition fill cannot silently become a crest where no resolved ridge exists.
- Crest profiles should stay asymmetric at launch: windward approach broader, lee side steeper, with low-amplitude curvature so parallel ridges do not read as perfectly straight stripes.
- The feature should remain readable across several chunks without overwriting the owning region identity or behaving like a full ridge-chain system.
- Launch tuning target is moderate uplift relative to the local prototype, usually several blocks rather than cliff-scale vertical relief.

## Runtime Ownership

- `build_dune_field_window(...)` should gather dune objects from a stable owner unit larger than a chunk.
- Neighboring chunks that overlap the same dune footprint must sample the same resolved dune objects.
- Resolved ownership should be deterministic from guide ownership alone and independent of generation request order.
- The feature-local window should keep only resolved objects relevant to the requested chunk window after owner-region resolve, so the later main-thread hook can stay thin.

## Surface Contract

- `sample_dune_field_apply_signal_from_window(...)` returns feature-local crest coverage, broad field coverage, crest height, support height, and a raise cap.
- `sample_dune_field_surface_from_window(...)` returns a target local surface with:
  - `target_surface_y`
  - `blend_weight`
  - `relief_spend`
  - `crest_coverage`
  - `field_coverage`
- Broad field support may raise the plain slightly between dunes, but only direct resolved crest coverage should drive the highest dune uplift.

## Temporary Guide Staging

- Shared `MesoGuideCell` does not yet expose dune-specific channels.
- Until the main thread adds authoritative dune guide emission, feature-local preview/debug wiring may stage dune input through the terrace-oriented fields:
  - `terrace_weight` as dune-field presence / strength
  - `terrace_step_height` as crest relief hint
  - `terrace_spacing_cells` as crosswind crest spacing hint
  - `terrace_heading_x/z` as dominant crest or wind alignment hint
- This staging contract is intentionally local to `dune_field` and should be replaced once shared dune atlas emission exists.

## Ecology Notes

- Later ecology can bias sparse scrub, exposed sediment, slip-face shelter, or dune-tolerant cover from the same resolved footprint.
- Hydrology and material hooks should read resolved dune objects as arid modulation rather than as wet lowland or corridor-closing terrain.

## Focused Tests

- deterministic resolved output for the same staged guide map and chunk
- stable world-space sampling across neighboring chunk windows
- flat response when dune guides are absent
- crest coverage narrower than total field coverage
- one dune-specific shape invariant for asymmetric windward vs lee behavior or repeated crest spacing

## Follow-up

- main-thread integration still needs shared exports and meso-apply orchestration wiring outside this ownership folder
- once shared dune channels exist, replace the temporary terrace-field staging contract with authoritative dune guide input
