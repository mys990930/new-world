# realize

## Role

- Own the top-level chunk generation loop and stone-only block fill.

## Responsibilities

- iterate chunk columns
- sample atlas inputs
- resolve the terrain profile
- compute `surface_y`
- write the pre-material terrain scaffold through the surface

## Notes

- The realization pass is still intentionally simple above the surface: everything above `surface_y` is air until later material and fluid passes are added.
- The filled block is currently `terrain_debug`, a neutral placeholder for pre-material terrain inspection.
- Probe helpers stop before block fill and let tooling inspect the same `sample -> profile -> surface_y` path without generating a full preview image.
