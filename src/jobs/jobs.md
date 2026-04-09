## jobs

### 역할

- 메인 frame/fixed loop에서 직접 돌리기 무거운 작업을 백그라운드 실행 단위로 분리한다.
- 상위 모듈이 제출한 요청을 worker 실행으로 연결하고, 완료 결과를 명시적으로 수거할 수 있게 노출한다.

### 책임

- job submission surface 제공
- pending/running/completed queue ownership
- worker thread/task 실행과 shutdown coordination
- job type별 world API routing
- 완료 결과 수집과 deterministic drain surface 제공
- 안전한 범위의 중복 요청 coalescing policy 유지

### 비책임

- 어떤 job이 필요한지 gameplay 차원에서 판단
- dirty chunk 판단
- draw call 수행
- ecs status 직접 수정
- live world source of truth 장기 소유

### 소유 데이터

- `JobConfig`
- `JobSystem`
- `JobRequest`
- `JobResult`
- `JobQueue`
- `WorkerContext`

### 공개 인터페이스

```rust
JobSystem::new(config: JobConfig) -> JobSystem
JobSystem::submit(request: JobRequest) -> Result<JobEnqueueOutcome, JobSubmitError>
JobSystem::submit_all(requests: impl IntoIterator<Item = JobRequest>) -> Result<(), JobSubmitError>
JobSystem::drain_completed() -> Vec<JobResult>
JobSystem::shutdown()
```

### 의존성

- `world`
- thread/task runtime abstraction
- logging

NOT:

- `platform`
- `renderer`
- `app`
- `ecs`

### 불변식

1. jobs는 live world state를 직접 들고 있지 않는다.
2. worker 입력은 가능하면 immutable snapshot 또는 value payload 기반으로 전달한다.
3. simulation job도 동일하게 snapshot 기반으로 계산한다.
4. 작업 결과는 메인 쪽에서 명시적으로 수거되기 전까지 completed queue에 보존된다.
5. 같은 coalesce key를 가진 요청은 안전한 경우에만 병합한다.
6. jobs는 결과를 계산하고 전달하지만, 그 결과의 gameplay 의미 해석은 상위 계층이 담당한다.

### 하위 모듈 목록 및 역할

- `config.md`: worker 개수, queue 정책, shutdown 정책 같은 실행 설정
- `request.md`: `JobRequest` variant와 worker-safe 입력 payload 계약
- `result.md`: `JobResult` variant와 완료/실패 결과 payload 계약
- `queue.md`: pending/running/completed queue와 coalescing 규칙
- `runtime.md`: `JobSystem` 소유 구조와 public API
- `worker.md`: worker 실행 단위, thread/task context, shutdown coordination
- `routing.md`: request variant를 `world`/`simulation` 작업으로 연결하는 dispatch 규칙

### 현재 구현 메모

- `jobs`는 이제 최소 Rust 구현이 들어와 있지만, 아직 전체 target spec보다 범위가 좁다.
- 현재 최소 구현은 `mod.rs + config.rs + request.rs + result.rs + queue.rs + runtime.rs + worker.rs + routing.rs`까지 연결되어 있다.
- 현재 지원 job은 `GenerateChunk`와 `BuildChunkMesh` 두 종류다.
- 두 request 모두 immutable snapshot/value payload와 함께 `Arc<BlockRegistry>`를 받아 worker에서 같은 block definition을 사용한다.
- worker 실행은 `std::thread + std::sync::mpsc` 기반의 최소 worker pool을 사용한다.
- `JobResult`는 `world::ChunkData` / `world::CpuMesh`까지만 들고 나오고, renderer upload 변환은 이후 app/ecs bridge 단계에서 연결한다.
- load/save/simulation job routing은 다음 단계에서 request/result variant와 함께 확장한다.
