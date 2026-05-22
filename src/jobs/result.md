# result

## Role

- Define the `JobResult` success/failure boundary.
- Fix the data shape that worker execution reports back to the main thread.

## Owned Data

### JobResult
- `CreateWorldProgress { root, completed_chunks, total_chunks }`
- `WorldCreated { root, manifest }`
- `ChunkLoaded { coord, chunk }`
- `ChunkGenerated { coord, chunk }`
- `ChunkUnloaded { coord }`
- `ChunkMeshBuilt { coord, mesh }`
- `MinimapChunkColumnBuilt { coord, patch }`
- `RegionClassResolved { area, classes }`
- `JobFailed { request, error }`

### JobError
- `Shutdown`
- `WorkerDisconnected { worker_id }`
- `ExecutionFailed { message }`

## Inputs

- worker success outputs
- worker progress snapshots for long-running requests
- worker execution failures
- request identity / correlation data

## Outputs

- immutable completed result payloads drained by the main thread
- diagnostic result labels for worker completion timing logs

## State Transition Rules

- each executed request produces exactly one final success or failure result
- long-running requests may emit progress results before the final success/failure result
- results remain immutable in the completed queue until drained
- final completed queue order stays deterministic; progress results are intermediate status events and do not complete or dedupe-clear a request
- diagnostic result labels must not require live world access

## Invariants

- `JobResult` does not mutate live world state directly
- failure results retain the original request so upper layers can clear dedupe state or retry later
- execution failures now cover fallible create-world and created-world chunk load paths
- diagnostic labels must keep enough identity to correlate slow worker output with the originating request/result kind

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
- create-world progress results carry lightweight chunk counters keyed by root path and are intended for app-owned loading UI only
- app still decides how to log or recover from `JobFailed`
- `ChunkUnloaded` is a lifecycle completion signal only; app still owns the actual `WorldCore` removal, renderer mesh removal, and minimap cache invalidation
- minimap chunk-column results are cache data only; app still owns how and when that cache is read by the render bridge
- region-classification results are cache data only; app/world own when resolved samples enter the live `WorldCore` cache
- worker diagnostics include result labels so slow minimap, mesh, load, and generation jobs can be separated in logs
