# heightfield/stats

`stats.rs` owns heightfield diagnostics, neighbor traversal helpers, and visible/water/river delta measurements.

Stats are observational and must not mutate column data or define generation policy.
