# heightfield/stats

`stats.rs` owns heightfield diagnostics, neighbor traversal helpers, and visible/water delta
measurements.

Stats are observational and must not mutate column data or define generation policy. River-specific
stats remain in the public struct for compatibility, but they should stay zero while macro/heightfield
river realization is reset.
