# coord

## Role

- Define block, chunk, and world coordinates used by `world`.
- Own the authoritative conversion rules between world-space block positions and chunk-local positions.

## Responsibilities

- define `ChunkCoord`, `LocalBlockCoord`, and `WorldBlockCoord`
- define chunk edge and volume constants
- define the metric contract for block and chunk size
- provide world <-> chunk/local conversion rules

## Owned Data

- `ChunkCoord`
- `LocalBlockCoord`
- `WorldBlockCoord`
- `BLOCKS_PER_METER`
- `BLOCK_SIZE_M`
- `CHUNK_EDGE`
- `CHUNK_EDGE_M`
- `CHUNK_VOLUME`

## Public Interface

```rust
world_to_chunk_local(pos: WorldBlockCoord) -> (ChunkCoord, LocalBlockCoord)
chunk_local_to_world(chunk: ChunkCoord, local: LocalBlockCoord) -> WorldBlockCoord
is_local_in_bounds(local: LocalBlockCoord) -> bool
```

## Invariants

1. The same `WorldBlockCoord` always resolves to the same `(ChunkCoord, LocalBlockCoord)`.
2. `LocalBlockCoord` always stays within `0..CHUNK_EDGE`.
3. World/chunk conversion must remain deterministic for negative coordinates.
4. The current metric contract is `1 block = 0.5m`.
5. With `CHUNK_EDGE = 32`, one chunk side equals `16m`.

## Notes

- Coordinate and scale rules are a world-level shared contract. Other modules should reuse them instead of introducing their own conversions.
- The current implementation uses `CHUNK_EDGE = 32`, so `CHUNK_VOLUME = 32 * 32 * 32 = 32768`.
