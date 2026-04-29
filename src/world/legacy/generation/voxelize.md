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
- a `VoxelizationPlan` is an `x/z` column plan with world-space terrain and water tops; callers may apply it to different vertical chunk coordinates that share the same `x/z`
- y-specific voxelization may emit a uniform air chunk immediately when the requested chunk range sits above every terrain/water top in the plan
- create-world style stack generation should reuse one plan for all requested `y` chunks instead of rebuilding upstream generation stages per vertical layer
