# heightfield/mapping

`mapping.rs` owns macro scalar to block-height conversion, contour-band resolve, snap helpers, and deterministic scalar interpolation primitives used by heightfield.

It must not decide terrain ownership, water presence, or river morphology.
