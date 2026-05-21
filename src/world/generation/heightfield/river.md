# heightfield/river

`river.rs` owns only heightfield post-processing for river columns: river water hint descent/suppression and the narrow same-context riverbed step guard across a heightfield tile.

River valley shape, bank lowering, core bed depth, and deterministic river-bed variation are upstream responsibilities baked into `MacroFieldSample.combined_macro_height` by `macro_field`. Heightfield must not reinterpret `river_shoulder_strength`, `river_bed_depth_hint`, or Q as a new terrain carve.

`river_core_strength` gates river water/terrain-kind eligibility. `river_bed_depth_hint` is preserved as a diagnostic and water-depth hint so the resolved macro bed can carry water, but it does not subtract terrain height in this stage.

After river water descent, active river neighbors with nearly identical core strength, flow hint, and river distance may lower only the higher already-resolved bed so same-context lower-channel cross-section steps stay within one block. This is a smoothing guard, not a river morphology pass.
