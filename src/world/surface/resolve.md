# resolve

## Role

- resolve one chunk-column surface plan from region ownership, smoothed terrain hints, hydrology output, and optional runtime season context
- hand voxelization quantized terrain / water tops plus chosen block keys without re-solving landform or water geometry

## Responsibilities

- map `RegionArchetype` to one launch-oriented `MaterialPolicyId`
- resolve an optional `SeasonalBiomeStateId` when runtime season context is available
- derive `CoverPhase` and small `CoverOverrideRule` hooks such as `snowy_grass`
- choose per-column:
  - top block key
  - filler block key
  - core block key
  - optional water or ice block key
  - filler depth
- quantize terrain top and standing-water top for the later block write stage

## Non-Responsibilities

- atlas or region classification itself
- meso placement
- hydrology carve or connected-water solving
- final block writes into `ChunkData`
- world calendar progression

## Current Contract

- region ownership stays one-hot per sampled column
- hydrology is allowed to override sediment and wet-surface expression
- coastal and marine contexts may promote sea-level standing water even when inland hydrology did not emit it
- the default `generate_chunk(...)` path currently uses no runtime season context, so seasonal state may stay `None`
- explicit runtime seasonal context remains the future integration point for `WorldCore` calendar ownership

## Public Surface

```rust
resolve_material_policy_for_archetype(archetype: RegionArchetype) -> MaterialPolicyId

resolve_chunk_surface_plan(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationV2Inputs,
    smoothed: &SmoothedPrototype,
    hydrology: &HydrologySolve,
) -> ChunkSurfacePlan

resolve_chunk_surface_plan_with_runtime(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationV2Inputs,
    smoothed: &SmoothedPrototype,
    hydrology: &HydrologySolve,
    runtime: Option<&SurfaceRuntimeContext>,
) -> ChunkSurfacePlan
```
