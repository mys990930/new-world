# pipeline

## Role

- define the stage order for graph-first world generation
- keep the new pipeline separate from the archived legacy generator while scaffolding proceeds

## Responsibilities

- name the graph-first generation stages
- define the initial generation configuration surface
- define column synthesis request/result shapes that later chunk voxelization can consume

## Non-Responsibilities

- actually generating Voronoi sites
- running noise functions
- solving hydrology
- placing blocks into `ChunkData`

## Stage Order

1. macro Voronoi graph
2. continuous region fields
3. land/ocean gradient
4. mountain gradient
5. hydrology graph
6. biome resolve
7. heightfield synthesis
8. voxel fill

## Invariants

1. chunk coordinates select output windows only; they do not define macro terrain identity.
2. graph and hydrology guides are solved before the base heightfield is committed to voxels.
3. the sea-level contract stays at world-space `y = 0` until `WorldMeta.generator_version` deliberately changes it.

## Current Status

- this module is a compile-time scaffold for the v2 pipeline
- legacy generation entrypoints are re-exported through `world::generation` for runtime compatibility during migration
