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
- bounded completed-result draining for interactive frame budgets
- diagnostic snapshots and slow worker timing logs
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
JobSystem::drain_completed_limit(max_results: usize) -> Vec<JobResult>
JobSystem::diagnostic_snapshot() -> JobSystemSnapshot
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
6. runtime region-classification resolves are background work so environment/HUD refresh does not generate atlas structure on the frame thread
7. progress results are intermediate status events and must not clear running/coalescing state for their request
8. limited drains preserve deterministic result order and keep undrained results buffered
9. diagnostics classify unload/minimap/region-classification work separately from load/generate/mesh work

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
- default runtime config uses up to two workers from available CPU parallelism so independent chunk load/mesh/minimap jobs can make progress while leaving CPU headroom for input and rendering
- job system startup and shutdown are logged with worker/queue configuration so hangs can be separated from missing window/surface startup
- the active request variants are `CreateWorld`, `LoadChunk`, `GenerateChunk`, `UnloadChunk`, `BuildChunkMesh`, `BuildMinimapChunkColumn`, and `ResolveRegionClassArea`
- `CreateWorld` delegates graph-first bounded created-world dump generation to `world`, including chunk storage and manifest writing
- `CreateWorld` can emit lightweight `CreateWorldProgress` snapshots before its final `WorldCreated`/`JobFailed` result; routing throttles those snapshots so UI feedback does not spam the main thread
- `UnloadChunk` routes through the jobs queue as a lightweight completion barrier; live world and renderer removal remain app-owned when `ChunkUnloaded` is drained
- gameplay can use limited result draining so bursty mesh completions do not force all renderer uploads into one frame
- slow worker jobs are logged by default; `NEW_WORLD_TRACE_JOBS=1` enables full worker start/finish tracing
- focused runtime region classification is queued through `ResolveRegionClassArea` and applied as a `WorldCore` cache update after the result is drained
- app and ECS still own result interpretation and runtime-world insertion after workers finish
