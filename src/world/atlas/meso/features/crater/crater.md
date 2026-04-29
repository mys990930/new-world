# crater

## Stage

- `launch`

## Identity

- placement family: `VolcanicField`
- hydrology coupling: `None`

## Summary

- Deterministic multi-chunk crater bowls with raised rims, shallow outer aprons, and stable meso-region ownership.

## Terrain Traits

- Atlas-side meso still owns only broad blended guides; crater runtime resolve owns the actual world-space crater objects.
- Runtime resolve must not rebuild a different crater guess per chunk or per sampled column.
- Neighboring chunks that overlap the same footprint must sample the same resolved crater set from the same meso-region-owned window.
- A resolved crater should keep bowl depth, rim uplift, and outer apron support as separate signals so later compositing can gate them independently.
- Launch tuning should read as a volcanic impact or collapse bowl rather than as a generic wet basin: the center cuts down, the rim rises above the nearby prototype, and the feature fades back into surrounding terrain over several chunks.
- Crater outlines should stay mildly irregular and asymmetric so they do not read as perfect circles at preview scale.
- Launch tuning should stay broad enough to survive the current `0.5m` block scale and later corridor / relief gating without collapsing into sub-meter noise.

## Runtime Contract

- Feature-local entrypoints:
  - `build_crater_window(...)`
  - `sample_crater_apply_signal_from_window(...)`
  - `sample_crater_surface_from_window(...)`
  - `debug_crater_candidates(...)`
- `build_crater_window(...)` resolves sparse world-space crater objects from a stable ownership unit larger than one chunk.
- `sample_crater_apply_signal_from_window(...)` returns crater-local bowl/rim/apron signal without deciding cross-feature policy.
- `sample_crater_surface_from_window(...)` returns one surface-oriented sample:
  - `target_surface_y`
  - `blend_weight`
  - `relief_spend`
  - `bowl_coverage`
  - `rim_coverage`
- Feature-local tests should keep resolve determinism, chunk-window ownership stability, seam continuity, flat-empty behavior, and one crater-specific shape invariant covered before shared wiring lands.

## Launch-Era Guide Assumption

- Shared meso generation does not yet emit a dedicated crater guide channel.
- Until the main thread adds that shared guide path, the crater runtime interprets existing `basin_*` and `hill_*` guide signals as a provisional volcanic proxy:
  - basin channels drive bowl placement and depth
  - hill channels help size or strengthen the rim
- This is intentionally a feature-local bridge, not the final atlas contract.

## Ecology Notes

- Later ecology can emphasize sparse pioneer cover, ash or scoria rims, exposed mineral floors, and localized drainage pockets that stop short of making the crater a full lake by default.
- Material policy remains outside this feature folder for now; crater runtime only resolves terrain shape.

## Follow-up

- main thread can later wire `build_crater_window(...)` and `sample_crater_surface_from_window(...)` into shared meso apply
- dedicated crater guide emission can replace the current basin or hill proxy once shared atlas selection expands
- future material and ecology hooks should read the same resolved crater ownership instead of re-detecting bowls from final height alone
