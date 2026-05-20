# heightfield/column

`column.rs` owns conversion of one `MacroFieldSample` into one `HeightfieldColumn`, including per-column terrain kind, height policy orchestration, and initial water hint assembly.

Ocean/lake ownership is resolved before river ownership at this layer, but reusable ocean/lake water-level and bed helpers live in `water.rs`. River post-processing belongs to `river.rs`.

River ownership is driven by `river_core_strength`, not the broad `river_shoulder_strength` or legacy `river_valley_strength` aggregate. Shoulder-only columns can keep land/bank relief context, but they must not become river water or receive core bed depth. When a shoulder-only column is immediately outside a strong lower-channel core, column conversion may apply bounded bank lowering so the bank slopes toward the water/bed instead of staying as an uncut wall.

Ocean/lake mouth bed hints are only applied when the source bed is already below the standing-water
level. An above-sea ocean-owned sample can keep river diagnostics, but crossing the river core
threshold by itself must not cut that column into a below-sea river mouth trench.

Column conversion preserves `river_distance_blocks` on `HeightfieldColumn` so tile-level river
post-processing can distinguish same-context lower-channel neighbors from unrelated river or
standing-water transitions.

The regression suite for this module lives in `column_tests.rs` and is included as a module-local `#[cfg(test)]` child of `column.rs`.
