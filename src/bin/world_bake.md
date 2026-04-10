# world_bake

## Role

- Generate a bounded chunk volume from a seed and persist the result to disk.
- Produce a manifest with stack summaries so preview-coordinate lookup can happen without regenerating chunks.

## Inputs

- `seed`
- chunk-space bake center
- horizontal bake radius
- vertical chunk bounds
- output directory

## Outputs

- baked chunk files under `<output>/chunks/`
- `<output>/manifest.toml`

## Current Flow

1. Build `WorldMeta` and `BlockRegistry`.
2. Iterate the requested `x/z` chunk stacks.
3. Generate every `y` chunk in that stack with `world::generation`.
4. Save each chunk through `world::storage`.
5. Summarize the stack relief and record it in the manifest.
6. Pick the highest-scoring stack as the default preview center.

## Default Vertical Window

- The current default bake window is chunk `y = -2..3`, matching the surface-inspection focus on roughly world `y > -40`.
