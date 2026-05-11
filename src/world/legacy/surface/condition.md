# condition

## Role

- define world-owned runtime surface-condition observations for textmode, renderer, and gameplay consumers

## Responsibilities

- represent chunk or atlas-cell surface state as structured data
- expose dry, wet, snow-covered, half-thawed snow, and frozen states
- keep numeric wetness, snow depth, and thaw fields normalized for observers
- attach observations to world-owned biome classification rather than app-local labels

## Non-Responsibilities

- console text formatting
- ecology event generation
- fixed tick scheduling
- block mutation policy

## Public Interface

```rust
SurfaceCondition
SurfaceConditionKind
SurfaceConditionScope
SurfaceConditionObservation
atlas_coord_for_chunk(coord: ChunkCoord) -> AtlasCoord
WorldCore::chunk_surface_condition(coord: ChunkCoord) -> SurfaceCondition
WorldCore::set_chunk_surface_condition(coord: ChunkCoord, condition: SurfaceCondition) -> Option<SurfaceCondition>
WorldCore::observe_chunk_surface_condition(coord: ChunkCoord) -> SurfaceConditionObservation
```

## Invariants

1. surface condition is world-readable structured state, not console text
2. chunk observations derive cell biome from world-owned region classification
3. missing chunk surface state reads as dry until simulation applies a more specific state
4. numeric fields are clamped to `0.0..=1.0`

## Related

- `../world.md`
- `surface.md`
- `../calendar.md`
- `../../../../temp-worker-docs/new-world-textmode-plan.md`
