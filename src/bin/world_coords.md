# world_coords

## Role

- Inspect a baked world manifest and print promising preview coordinates without regenerating the world.

## Inputs

- baked world directory
- optional number of top candidates to print

## Outputs

- manifest summary in stdout
- top stack candidates sorted by stored score

## Current Flow

1. Load `manifest.toml`.
2. Print bake bounds and the stored default preview center.
3. Print the highest-scoring chunk-stack coordinates for preview selection.
