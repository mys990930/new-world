# core

## Role

- Own the loaded world state and top-level world API.
- Keep other modules interacting with world state through `WorldCore`.

## Responsibilities

- define `WorldCore`
- hold `WorldMeta`
- hold immutable `BlockRegistry`
- own the loaded chunk map
- manage chunk insert/remove
- provide block/chunk read APIs
- expose top-level entry points for edit/query/storage/meshing-facing operations
- expose loaded block-grid raycast entry points
- expose loaded chunk bounds for runtime helpers that need search ranges

## Non-Responsibilities

- fixed tick scheduling
- jobs orchestration
- gameplay command interpretation
- renderer draw/upload
- generation execution policy

## Owned Data

### WorldCore
- `meta`
- `block_registry`
- loaded chunk map keyed by `ChunkCoord`

## Inputs

- `WorldMeta`
- `Arc<BlockRegistry>`
- loaded/generated `ChunkData`
- block/chunk queries
- explicit `WorldEdit`
- `Ray3` and max ray distance

## Outputs

- chunk presence / reference results
- inserted or removed `ChunkData`
- `EditResult`
- snapshots and query results
- `RaycastHit`
- loaded chunk bounds

## Public Interface
```rust
WorldCore::new(meta: WorldMeta, block_registry: Arc<BlockRegistry>) -> WorldCore
WorldCore::block_registry(&self) -> &BlockRegistry
WorldCore::block_registry_handle(&self) -> Arc<BlockRegistry>
WorldCore::meta(&self) -> &WorldMeta

WorldCore::has_chunk(coord: ChunkCoord) -> bool
WorldCore::insert_chunk(coord: ChunkCoord, chunk: ChunkData)
WorldCore::remove_chunk(coord: ChunkCoord) -> Option<ChunkData>
WorldCore::loaded_chunk_bounds(&self) -> Option<(ChunkCoord, ChunkCoord)>

WorldCore::get_block(pos: WorldBlockCoord) -> Option<BlockId>
WorldCore::get_chunk(coord: ChunkCoord) -> Option<&ChunkData>
WorldCore::get_chunk_mut(coord: ChunkCoord) -> Option<&mut ChunkData>
WorldCore::snapshot_chunk(coord: ChunkCoord) -> Option<ChunkSnapshot>

WorldCore::apply_edit(edit: WorldEdit) -> EditResult
WorldCore::raycast_blocks(ray: Ray3, max_distance: f32) -> Option<RaycastHit>
```

## Invariants

- the loaded chunk map is owned by `WorldCore`
- other modules do not mutate raw chunk storage directly
- `BlockRegistry` interpretation remains separate from raw chunk ids
- raycasts operate on loaded chunks only

## Related Modules

- `meta.md`
- `coord.md`
- `chunk.md`
- `edit.md`
- `query.md`
- `generation.md`
- `storage.md`
- `created.md`
- `registry.md`

## Notes

- `loaded_chunk_bounds()` exists specifically to support app/ECS helpers such as safe spawn placement without leaking the raw chunk map
- created-world loading still inserts chunks through `WorldCore::insert_chunk(...)`; `world` owns the in-memory source of truth regardless of how a chunk was acquired
