# result

## Role

- Define the `JobResult` success/failure boundary.
- Fix the data shape that worker execution reports back to the main thread.

## Owned Data

### JobResult
- `WorldCreated { root, manifest }`
- `ChunkLoaded { coord, chunk }`
- `ChunkGenerated { coord, chunk }`
- `ChunkMeshBuilt { coord, mesh }`
- `MinimapChunkColumnBuilt { coord, patch }`
- `JobFailed { request, error }`

### JobError
- `Shutdown`
- `WorkerDisconnected { worker_id }`
- `ExecutionFailed { message }`

## Inputs

- worker success outputs
- worker execution failures
- request identity / correlation data

## Outputs

- immutable completed result payloads drained by the main thread

## State Transition Rules

- each executed request produces exactly one success or failure result
- results remain immutable in the completed queue until drained
- completed queue order stays deterministic

## Invariants

- `JobResult` does not mutate live world state directly
- failure results retain the original request so upper layers can clear dedupe state or retry later
- execution failures now cover fallible create-world and created-world chunk load paths

## Non-Responsibilities

- interpreting gameplay meaning
- mutating world source-of-truth state
- uploading renderer resources

## Related Modules

- `queue.md`
- `worker.md`
- `../ecs/jobs.md`

## Notes

- create-world and created-world chunk load now use `ExecutionFailed { message }` when directory, manifest, disk read, or decode work fails
- app still decides how to log or recover from `JobFailed`
- minimap chunk-column results are cache data only; app still owns how and when that cache is read by the render bridge
