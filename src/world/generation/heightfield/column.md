# heightfield/column

`column.rs` owns conversion of one `MacroFieldSample` into one `HeightfieldColumn`, including per-column terrain kind, block-height resolve, and initial water hint assembly.

Ocean/lake ownership is resolved before river ownership at this layer, but reusable ocean/lake water-level and bed helpers live in `water.rs`. River post-processing belongs to `river.rs`.

River ownership is driven by `river_core_strength`, not the broad `river_shoulder_strength` or legacy `river_valley_strength` aggregate. Shoulder-only columns can keep diagnostic river context, but they must not become river water or receive core bed depth. Terminal estuary fan columns may also become local river-water continuation when macro_field provides `estuary_water_strength`; `estuary_water_depth_hint` can raise only the water-depth hint above a slope-limited fan bed. This affects water/terrain-kind and water-depth eligibility for above-sea mouth columns and must not add heightfield-local terrain carve. Column conversion must not apply new river bed, bank, or shoulder carve; that terrain shape must already be present in `combined_macro_height`.
When Perlin is enabled, river core columns can perturb the pre-contour bed source by a small bounded amount. This is a detail/noise pass over the macro-resolved bed, not a Q-driven carve or ownership change.

Ocean/lake mouth bed hints do not cut terrain in this layer. An above-sea ocean-owned sample can keep river diagnostics, but crossing the river core threshold by itself must not cut that column into a below-sea river mouth trench. Only explicit `estuary_water_strength` can opt an above-sea ocean-owned mouth column into river water continuity, and only explicit `estuary_water_depth_hint` can decouple that continuation's water depth from the shallow fan bed-depth hint.

Column conversion preserves `river_distance_blocks` on `HeightfieldColumn` so tile-level river
post-processing can distinguish same-context lower-channel neighbors from unrelated river or
standing-water transitions.

The regression suite for this module lives in `column_tests.rs` and is included as a module-local `#[cfg(test)]` child of `column.rs`.
