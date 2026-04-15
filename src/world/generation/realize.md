# realize

## Role

- Own the top-level chunk generation loop and layered block fill.

## Responsibilities

- iterate chunk columns
- consume the prepared smoothed surface field
- pick a stone-core ceiling
- classify the column fill profile
- write stone, sediment/topsoil, and sea water blocks

## Notes

- The current realization pass follows the original first-pass contract: `stone` core, atlas-informed sediment/topsoil, and sea water up to `y = 0`.
- Ground relief now comes from a blended profile surface that is smoothed before hydrology, so coast/plain/upland/ridge boundaries do not resolve as single-step profile cliffs or noisy block jitter.
- Near shore, coast classification now also uses low-frequency boundary noise and an emergent-shore rule, so shelf/land transitions do not stay locked to a perfectly smooth atlas bilerp contour.
- Near shore, coast classification wins over river-bed classification so beaches remain visible instead of collapsing entirely into river material.
- Sea water is now reserved for `DeepOcean` / `Shelf` columns; coast columns clamp to at least `y = 0` and stay dry unless a later river/estuary pass explicitly adds water.
- Strong river signals now carve floodplains/channels out of the smoothed terrain, using local concavity to favor real low points instead of painting equally across a whole riverine band.
- Atlas structure guides now feed the same carve, so explicit nearby river paths can dominate centerline placement instead of relying only on per-column meander noise.
- Shallow-ocean and river sediment are now chosen once per column from low-frequency sediment fields, so `sand` / `mud` / `gravel` read as broad patches instead of per-block speckle.
- River fill profiles keep river-bed material on the surface instead of reverting to grass just because the local channel is not submerged.
- The current hydrology carve is now hybrid: atlas-owned river path proximity drives the main channel when present, while older scalar river signals and local concavity remain as support.
- The current hydrology pass now also uses downstream progress from atlas river segments to bias river stage and lower downstream water surfaces more consistently along the branch.
- The next revision should add explicit confluences and reduce the remaining fallback dependence on scalar meander noise.
- Vegetation and ecology are still deferred.
- Probe helpers stop before block fill and let tooling inspect the same `sample -> profile -> surface_y` path without generating a full preview image.
