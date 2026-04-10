# realize

## Role

- Own the top-level chunk generation loop and layered block fill.

## Responsibilities

- iterate chunk columns
- sample atlas inputs
- resolve the terrain profile
- compute `surface_y`
- pick a stone-core ceiling
- classify the column fill profile
- write stone, sediment/topsoil, and sea water blocks

## Notes

- The current realization pass follows the original first-pass contract: `stone` core, atlas-informed sediment/topsoil, and sea water up to `y = 0`.
- Near shore, coast classification wins over river-bed classification so beaches remain visible instead of collapsing entirely into river material.
- Strong river signals now carve floodplains/channels out of the base terrain and can place inland river water above sea level.
- Submerged river surfaces use river-bed material instead of the exposed-land grass rule.
- Vegetation and ecology are still deferred.
- Probe helpers stop before block fill and let tooling inspect the same `sample -> profile -> surface_y` path without generating a full preview image.
