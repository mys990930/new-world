# realize

## Role

- Own the top-level chunk generation loop and stone-only block fill.

## Responsibilities

- iterate chunk columns
- sample atlas inputs
- resolve the terrain profile
- compute `surface_y`
- write `stone` through the surface

## Notes

- The realization pass is still intentionally simple above the surface: everything above `surface_y` is air until later material and fluid passes are added.
