# heightfield/river

`river.rs` owns only heightfield post-processing for river columns: river water hint descent/suppression and the narrow same-context riverbed step guard across a heightfield tile.

River valley shape, bank lowering, core bed depth, and deterministic river-bed variation are upstream responsibilities baked into `MacroFieldSample.combined_macro_height` by `macro_field`. Heightfield must not reinterpret `river_shoulder_strength`, `river_bed_depth_hint`, or Q as a new terrain carve.

`river_core_strength` gates ordinary river water/terrain-kind eligibility. Terminal estuary continuation is the narrow exception: when macro_field provides `estuary_water_strength`, an above-sea mouth column may carry river water even if it is ocean-owned. `river_bed_depth_hint` is preserved as a diagnostic and water-depth hint so the resolved macro bed can carry water, but it does not subtract terrain height in this stage.
River water height is derived from the already resolved macro bed: the water column starts at that integer bed and rises by the depth hint, with sea level as a lower bound for the surface. It must not be solved from the uncarved macro source height or use heightfield-local terrain downcut.
Before the descent pass, river water is capped by adjacent local land/coast banks when such a bank is at or above sea level. Same-flow river-core neighbors pool to the same lateral water surface, so high-Q rivers do not form stepped water blocks above their core bed.

After river water descent, active river neighbors with nearly identical core strength, flow hint, and river distance may lower only the higher already-resolved bed so same-context lower-channel cross-section steps stay within one block. This is a smoothing guard, not a river morphology pass.
