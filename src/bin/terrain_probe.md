# terrain_probe

## Role

- Print atlas-to-generation diagnostics for one chunk and one selected column.

## Inputs

- `seed`
- `--chunk-x <i32>`
- `--chunk-z <i32>`
- `--local-x <u8>`
- `--local-z <u8>`

## Outputs

- chunk-scale surface summary
- terrain-profile counts
- selected-column atlas bilerp values
- resolved generation profile and `surface_y`

## Notes

- This tool is intended for terrain tuning before material layering.
- It reports block-space and meter-space values using the current `1 block = 0.5m` contract.
