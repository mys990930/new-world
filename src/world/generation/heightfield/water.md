# heightfield/water

`water.rs` owns column-local ocean/lake water-level and standing-water bed helper policy.

It does not solve river continuity or river water fill. In the current reset state, river hints must
not override ocean/lake water policy or create standalone water columns.
