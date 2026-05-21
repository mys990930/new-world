# heightfield/river

`river.rs` owns the heightfield-local interpretation of macro-field river hints: bounded bed/bank relief helpers, river water hint descent/suppression, and a narrow same-context riverbed continuity guard across a heightfield tile.

`river_core_strength` gates river bed/water behavior. `river_shoulder_strength` gates only the non-water bank/shoulder relief around the selected river corridor. Near the core threshold, high-shoulder non-water banks receive a bounded lowering toward the channel so the lower river does not read as a vertical wall at the first river column. `river_valley_strength` remains a compatibility diagnostic aggregate and is not used to create river water.

Inside the selected core, `river_core_strength` also drives a center-weighted bed profile. The profile is zero at the configured river-water threshold and ramps toward the core center, adding bounded downcut before final snap while leaving the existing edge/bank thresholds intact. Deterministic bed relief remains multi-scale, but the positive raise allowance fades toward the core center so random detail cannot make the center read consistently shallower than near-edge core samples.

It does not own broad river valley carving or final water solving. It may adjust or suppress river water hints for continuity. After water descent, active river neighbors with nearly identical core strength, flow hint, and river distance may downcut only the higher bed so same-context lower-channel cross-section steps stay within one block; this guard must not move water surfaces, reinterpret hydrology topology, or use standing-water/ocean transitions as riverbed evidence.
