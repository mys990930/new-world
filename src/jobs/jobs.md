## jobs

### Role

- execute heavy work asynchronously outside the main frame loop
- translate upper-layer job requests into worker execution and completed results

### Responsibilities

- job submission surface
- pending/running/completed queue ownership
- worker thread execution and shutdown coordination
- request routing into world operations
- deterministic completed-result draining
- safe request coalescing

### Non-Responsibilities

- deciding gameplay meaning
- mutating ECS state directly
- mutating the live world source of truth directly
- issuing draw calls

### Owned Data

- `JobConfig`
- `JobSystem`
- `JobRequest`
- `JobResult`
- `JobQueue`
- `WorkerContext`

### Public Interface
```rust
JobSystem::new(config: JobConfig) -> JobSystem
JobSystem::submit(request: JobRequest) -> Result<JobEnqueueOutcome, JobSubmitError>
JobSystem::submit_all(requests: impl IntoIterator<Item = JobRequest>) -> Result<(), JobSubmitError>
JobSystem::drain_completed() -> Vec<JobResult>
JobSystem::shutdown()
```

### Dependencies

- `world`
- thread/task runtime abstraction
- logging

NOT:

- `platform`
- `renderer`
- `app`
- `ecs`

### Invariants

1. jobs do not own the live runtime world
2. worker inputs are immutable snapshot/value payloads
3. results are explicit and immutable until drained
4. the current runtime supports both created-world chunk load and procedural generation as acquisition paths
5. minimap chunk-column derivation is also treated as heavy background work and should not require live-world scanning on the main thread every frame

### Submodules

- `config.md`: worker count, queue capacity, shutdown policy
- `request.md`: `JobRequest` variants and worker-safe payloads
- `result.md`: `JobResult` variants and failure surface
- `queue.md`: pending/running/completed queue and coalescing rules
- `runtime.md`: `JobSystem` ownership and public API
- `worker.md`: worker execution scope and shutdown coordination
- `routing.md`: dispatch from request variants to world operations

### Current Implementation Notes

- the current worker pool still uses `std::thread + std::sync::mpsc`
- the active request variants are `CreateWorld`, `LoadChunk`, `GenerateChunk`, `BuildChunkMesh`, and `BuildMinimapChunkColumn`
- app and ECS still own result interpretation and runtime-world insertion after workers finish
