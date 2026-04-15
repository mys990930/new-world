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
- map operation success/failure into `JobResult`

## Non-Responsibilities

- queue state management
- worker lifecycle management
- gameplay rule interpretation
- mutating the live runtime world directly

## Inputs

- `JobRequest`
- world-side shared APIs and data types

## Outputs

- `JobResult`

## Process

1. match the request variant
2. call the corresponding world API
3. convert the outcome into the matching `JobResult`

## Invariants

- routing does not manage queue state directly
- routing only uses immutable payloads carried by the request
- routing stays on documented shared world APIs, not runtime internals

## Related Modules

- `request.md`
- `result.md`
- `worker.md`
- `../world/world.md`

## Notes

- the current routing surface now covers create-world directory creation, created-world chunk load, procedural generation, meshing, and snapshot-based minimap chunk-column derivation
- create-world and created-world load routes are intentionally fallible worker paths in the current runtime
