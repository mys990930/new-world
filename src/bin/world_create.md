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
2. Build or reuse generation input bundles keyed by generation atlas area.
3. Iterate the requested `x/z` chunk stacks, with independent stacks eligible for parallel generation.
4. Solve the chunk-local generation surface / voxelization plan once per `x/z` stack.
5. Voxelize each requested `y` chunk from that shared plan and save it through `world::storage`.
6. Summarize the stack relief from the resolved plan rather than rescanning realized blocks.
7. Pick the highest-scoring stack as the default preview center.

## Stack Generation Contract

- The expensive generation stages before final block writes are `x/z` surface work:
  - atlas input assembly
  - realization field
  - river corridor window
  - base heightfield
  - meso apply
  - smoothing
  - hydrology
  - surface/material resolve
  - voxelization plan assembly
- These stages must be evaluated once for a chunk column and then reused for all requested vertical chunks in that stack.
- The final y-specific step is voxelization from the shared plan into a `ChunkData` with the requested `ChunkCoord`.
- A vertical chunk whose y range is above every terrain/water top in the shared plan may be emitted directly as a uniform air chunk.
- Stack summaries should derive from the same surface/voxelization plan used for block fill so preview-center selection does not require a separate `WorldCore` block scan.
- Parallel stack generation must keep output deterministic for the same seed and bounds:
  - each stack owns disjoint chunk paths
  - manifest sorting remains deterministic after generation
  - reused input bundles must match `chunk_generation_input_area(...)`

## Default Vertical Window

- The current default create-world window is chunk `y = -2..3`, matching the surface-inspection focus on roughly world `y > -40`.
