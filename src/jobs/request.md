# request

## Role

- Define the worker-safe `JobRequest` payload boundary.
- Make explicit what upper layers are allowed to ask the jobs system to do.

## Owned Data

### JobRequest
- `CreateWorld { root, config, registry }`
- `LoadChunk { root, coord }`
- `GenerateChunk { coord, meta, registry }`
- `BuildChunkMesh { center, neighbors, registry }`
- `BuildMinimapChunkColumn { coord, chunks, registry }`

### Request identity / coalesce key
- path-based dedupe identity for `CreateWorld`
- chunk-coordinate-based dedupe identity for chunk-scoped work

## Inputs

- `ChunkCoord`
- `CreateWorldConfig`
- created-world root path
- immutable `ChunkSnapshot`
- immutable chunk-column `Vec<ChunkSnapshot>`
- `NeighborChunks`
- `WorldMeta`
- `Arc<BlockRegistry>`

## Outputs

- owned worker payloads
- queue-facing request identity / coalesce keys

## State Transition Rules

- requests are created from snapshot/value payloads, never live world borrows
- block registry remains immutable shared config behind `Arc`
- queue entry computes a coalesce key up front
- request payload becomes immutable once handed to a worker

## Invariants

- `JobRequest` must be safe to move across worker boundaries
- request payload must not keep live world references
- coalescing only applies when duplicate work is semantically safe

## Non-Responsibilities

- executing the request
- applying results back to ECS/world/renderer
- choosing queue policy

## Related Modules

- `queue.md`
- `routing.md`
- `../ecs/jobs.md`
- `../world/created.md`

## Notes

- the current chunk acquisition path now distinguishes created-world load from procedural generation
- create-world requests coalesce on destination root path so duplicate button presses do not enqueue duplicate directory creation
- chunk-scoped coalescing still keys on chunk coordinate because the current runtime owns only one active world session
- minimap chunk-column rebuilds coalesce on chunk-column `x/z`; app cache keeps a dirty-after-pending bit so later chunk arrivals can schedule one more rebuild if a stale worker result wins the race
