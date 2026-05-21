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
2. Build one graph-first voxel plan for the requested x/z chunk footprint.
3. The plan internally runs graph, macro map, hydrology, noisy boundary, macro field,
   Perlin-enabled heightfield, surface plan, and pixelize once for that bounded footprint.
4. Iterate the requested `x/z` chunk stacks, with independent stacks eligible for parallel generation.
5. Voxelize each requested `y` chunk from the shared graph-first plan and save it through
   `world::storage`.
6. Summarize the stack relief from the resolved plan rather than rescanning realized blocks.
7. Pick the highest-scoring stack as the default preview center.

## Stack Generation Contract

- The expensive generation stages before final block writes are graph-first `x/z` footprint work:
  - Voronoi graph patch
  - macro map
  - selected hydrology
  - noisy boundary
  - macro field tile
  - Perlin-enabled heightfield
  - surface plan area
  - pixelized chunk area
  - graph-first voxel plan assembly
- These stages must be evaluated once for the requested x/z footprint and then reused for all requested vertical chunks.
- The final y-specific step is voxelization from the shared plan into a `ChunkData` with the requested `ChunkCoord`.
- A vertical chunk whose y range is above every terrain/water top in the shared plan may be emitted directly as a uniform air chunk.
- Stack summaries should derive from the same surface/voxelization plan used for block fill so preview-center selection does not require a separate `WorldCore` block scan.
- Parallel stack generation must keep output deterministic for the same seed and bounds:
  - each stack owns disjoint chunk paths
  - manifest sorting remains deterministic after generation
  - every y stack reuses the same graph-first `GraphFirstVoxelPlan`

## Fill Policy

- Perlin micro relief is enabled through the default graph-first build config.
- Surface/material policy is resolved before voxel fill and contributes top, subsurface, base, and
  underwater top block keys.
- Water columns write the registry `water` block up to `water_y`.
- Vegetation placement is still stubbed.
- Air remains air above terrain/water.

## Default Vertical Window

- The current default create-world window is chunk `y = -2..3`, matching the surface-inspection focus on roughly world `y > -40`.
