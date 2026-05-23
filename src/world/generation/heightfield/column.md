# heightfield/column

`column.rs` owns conversion of one `MacroFieldSample` into one `HeightfieldColumn`, including per-column terrain kind, block-height resolve, and initial water hint assembly.

Ocean/lake ownership is resolved before river ownership at this layer, but reusable ocean/lake water-level and bed helpers live in `water.rs`. River post-processing belongs to `river.rs`.

River ownership follows the project river morphology vocabulary. `RiverValley` is the broad lowland made by the river and `river_shoulder_strength` is the inner bank/slope context, but neither creates river water in heightfield. `RiverBed` is the macro-lowered, exposed bed/gravel/deposition tier around the water-filled channel. `RiverCore` is only the still-lower active channel inside that bed: the sample must be selected by the river/estuary water profile, macro_field must have actually lowered it by at least one block (`macro_elevation > combined_macro_height`), and the profile strength must pass the flow-scaled active-core cutoff. This keeps the flat bed tier from being swallowed by core while still allowing high-Q channels to have a broader U-shaped core. Unlowered threshold hints remain ordinary terrain. Terminal estuary fan hints can mark exposed `RiverBed` at the fan edge, but the fan center can become water-filled `RiverCore` when `estuary_water_strength` passes the same active-core cutoff and the macro-resolved bed reaches below sea level. Column conversion must not apply new river bed, bank, or shoulder carve; that terrain shape must already be present in `combined_macro_height`.
When Perlin is enabled, river core columns can perturb the pre-contour bed source by a small bounded amount. This is a detail/noise pass over the macro-resolved bed, not a Q-driven carve or ownership change.

Ocean/lake mouth bed hints do not cut terrain in this layer. An above-sea ocean-owned sample can keep river diagnostics, but crossing the river threshold by itself must not cut that column into a below-sea river mouth trench. Explicit `estuary_water_strength` may keep estuary fan bed context at the edge or restore water-filled core at the fan center; it still only reads the already-baked `combined_macro_height`. Estuary-dominant fan water uses sea level (`y = 0`) rather than the upstream river fill-depth, except where an actual selected river core is still present as the local river/estuary handoff.

Column conversion preserves `river_distance_blocks` on `HeightfieldColumn` so tile-level river
post-processing can distinguish same-context lower-channel neighbors from unrelated river or
standing-water transitions.

The regression suite for this module lives in `column_tests.rs` and is included as a module-local `#[cfg(test)]` child of `column.rs`.
