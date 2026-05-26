# heightfield/river

`river.rs` is currently a reset guard, not a river realization module.

`apply_river_water_descent` keeps the public post-processing hook in place while macro/heightfield
river terrain is being redesigned. The pass clears river strengths, flow/depth/roughness diagnostics,
`river_core_water_height_blocks`, and any accidental `RiverCore` / `RiverBed` terrain kind left by
legacy callers.

This module must not:

- solve river water descent,
- carve river bed/bank/shoulder terrain,
- reinterpret Q or `river_core_depth_hint`,
- override ocean/lake water policy.
