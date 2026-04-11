# terrain_corridor

## Role

- Find an atlas-scale corridor that runs from ocean toward mountain terrain and visualize its chunk-scale height progression.

## Inputs

- `seed`
- `--radius-cells <i32>`
- `--min-length-cells <u32>`
- `--max-length-cells <u32>`
- `--output <path>`

## Outputs

- stdout summary of the best corridor
- a PNG chart showing chunk-scale height progression along that corridor
- a suggested `chunk_preview` command for a midpoint visual check

## Notes

- The search currently checks axis-aligned atlas corridors.
- The chart is meant to answer “does sea -> coast -> inland -> mountain rise coherently?” more directly than a single isometric screenshot can.
