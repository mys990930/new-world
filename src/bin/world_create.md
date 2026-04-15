# world_create

## Role

- Generate a bounded chunk volume from a seed and persist the result to disk.
- Produce a manifest with stack summaries so preview-coordinate lookup can happen without regenerating chunks.

## Inputs

- `seed`
- chunk-space create-world center
- horizontal create-world radius
- vertical chunk bounds
- output directory

## Outputs

- created-world chunk files under `<output>/chunks/`
- `<output>/manifest.toml`

## Current Flow

1. Build `WorldMeta` and `BlockRegistry`.
2. Iterate the requested `x/z` chunk stacks.
3. Generate every `y` chunk in that stack with `world::generation`.
4. Save each chunk through `world::storage`.
5. Summarize the stack relief and record it in the manifest.
6. Pick the highest-scoring stack as the default preview center.

## Default Vertical Window

- The current default create-world window is chunk `y = -2..3`, matching the surface-inspection focus on roughly world `y > -40`.
