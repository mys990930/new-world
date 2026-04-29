# routing

## Role

- Map `JobRequest` variants to concrete worker-side world operations.
- Keep the request-to-operation dispatch rules in one place.

## Responsibilities

- `CreateWorld` -> created-world directory creation
- `LoadChunk` -> created-world storage load
- `GenerateChunk` -> procedural generation
- `BuildChunkMesh` -> meshing
- `BuildMinimapChunkColumn` -> snapshot-based top-down chunk-column derivation
- `ResolveRegionClassArea` -> atlas fields/structure generation plus region classification
- map operation success/failure into `JobResult`
- throttle long-running operation progress into lightweight `JobResult` snapshots

## Non-Responsibilities

- queue state management
- worker lifecycle management
- gameplay rule interpretation
- mutating the live runtime world directly

## Inputs

- `JobRequest`
- world-side shared APIs and data types

## Outputs

- intermediate progress `JobResult`
- final success/failure `JobResult`

## Process

1. match the request variant
2. call the corresponding world API
3. forward progress snapshots when the world API reports them
4. convert the outcome into the matching final `JobResult`

## Invariants

- routing does not manage queue state directly
- routing only uses immutable payloads carried by the request
- routing stays on documented shared world APIs, not runtime internals
- routing progress reports must not imply request completion

## Related Modules

- `request.md`
- `result.md`
- `worker.md`
- `../world/world.md`

## Notes

- the current routing surface now covers create-world directory creation, created-world chunk load, procedural generation, meshing, snapshot-based minimap chunk-column derivation, and background region-classification resolves
- create-world and created-world load routes are intentionally fallible worker paths in the current runtime
- create-world progress is throttled by completed chunk count before being forwarded to the worker report channel
- create-world routing logs start and finish metadata, including root, seed, radius, vertical range, chunk count, and stack count
