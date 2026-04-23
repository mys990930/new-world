# voxelize

## Role

- own the last step that turns a resolved chunk surface plan into actual `ChunkData` block placement

## Current Types

- `VoxelizationColumnPlan`
- `VoxelizationPlan`

## Notes

- the current runtime path now consumes a pre-resolved `ChunkSurfacePlan`
- voxelize still owns the final write into `ChunkData`
- voxelize should quantize hydrology's carved surface and connected water result into blocks; it should not perform a second independent channel or basin carve
- deposition hints such as `gravel_bar_strength` should continue to influence earlier surface-plan choice, not trigger a second transport solve here
