# smoothing

## Role

- own post-meso smoothing and local refinement while preserving corridor and ridge intent

## Responsibilities

- accept the meso-applied chunk surface as the pre-smoothing baseline
- spend only a small fraction of each column's remaining relief budget on local relaxation
- suppress smoothing near carried river corridors so valley seats and exits survive into hydrology
- avoid introducing new shared-edge seams by leaving chunk-border columns unchanged in the minimal implementation
- derive local slope and signed concavity hints from the smoothed surface for later hydrology / voxelization stages

## Current Types

- `SmoothedColumn`
- `SmoothedPrototype`

## Current Interface

```rust
build_chunk_smoothed_prototype(
    chunk: ChunkCoord,
    corridor_window: &ChunkCorridorWindow,
    meso: &MesoAppliedPrototype,
) -> SmoothedPrototype
```

## Current Minimal Implementation

- runs one deterministic in-chunk constrained smoothing pass after `meso_apply`
- relaxes columns only toward their orthogonal-neighbor average and clamps the applied adjustment
- reduces smoothing strength near corridors and around already-sharp local landforms
- uses the shared corridor-axis sampler for corridor preservation so smoothing protects the same branch-continuous valley axis that prototype and hydrology see
- preserves chunk-edge continuity by leaving border columns unchanged
- emits one `SmoothedColumn` per chunk column with:
  - smoothed `height`
  - forwarded `remaining_relief_budget`
  - `local_slope`
  - signed `concavity`
  - refined `material_support` axes for wetness, exposure, sediment, and stable soil/cover

Positive `concavity` means the column sits below its orthogonal-neighbor average and therefore reads as locally bowl-shaped. Negative `concavity` means the column stands above its neighbors and therefore reads as locally ridge-like.

`material_support` is the final generation-owned broad environment evidence handed to surface
resolve. Smoothing may sanitize and forward this support, but it should not derive new cover
categories from every local slope, concavity, or height step. Local terrain hints can still expose
rock or hydrology sediment through feature-owned rules, while ordinary cover should remain stable
inside the same visible environment.

## Notes

- this is intentionally a minimal launch-safe pass, not a full erosion or weathering solver
- later revisions may widen the neighborhood, consume feature-local protection masks, and feed stronger hydrology-specific refinement rules
