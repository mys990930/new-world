# request

## Role

- Define the worker-safe `JobRequest` payload boundary.
- Make explicit what upper layers are allowed to ask the jobs system to do.

## Owned Data

### JobRequest
- `LoadChunk { root, coord }`
- `GenerateChunk { coord, meta, registry }`
- `BuildChunkMesh { center, neighbors, registry }`

### Request identity / coalesce key
- chunk-coordinate-based dedupe identity

## Inputs

- `ChunkCoord`
- baked-world root path
- immutable `ChunkSnapshot`
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
- `../world/baked.md`

## Notes

- the current chunk acquisition path now distinguishes baked load from procedural generation
- coalescing still keys on chunk coordinate because the current runtime owns only one active world session
