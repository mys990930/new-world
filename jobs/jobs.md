## jobs

### 역할

- 메인 프레임 루프에서 직접 돌리기엔 무거운 작업을 백그라운드에서 async로 처리함
- 무슨 작업이 필요한지 판단하지는 않고, 요청된 작업을 실제로 수행함

### 책임

- job request queue administration
- worker thread/task execution
- job result collection
- job type-specific routing
- race condition prevention

### 비책임

- dirty chunk judgment
- draw call
- ecs status modification

### 데이터

#### JobRequest

- LoadChunk(coord)
- GenerateChunk(coord)
- BuildChunkMesh(coord, snapshot)
- SaveChunk(coord, snapshot)
- SimulateSubsystemTick(subsystem, tick, region, snapshot)
- SimulateRegionTick(tick, region, bundle)

#### JobResult

- ChunkLoaded(coord, chunk)
- ChunkGenerated(coord, chunk)
- ChunkMeshBuilt(coord, mesh)
- ChunkSaved(coord)
- JobFailed(…)
- SimulationStepped(subsystem, tick, result)

#### JobQueue

- PendingRequest
- RunningRequest
- CompletedRequestQueue

#### WorkerContext

- ThreadPool
- ChannelSender/ChannelReceiver
- ShutdownFlag

### 유스케이스

- 청크 로드
    - coord를 받아서 파일에서 읽기
    - 없으면 실패 또는 generate fallback
- 청크 생성
    - seed, coord 기반 절차 생성
    - 결과 ChunkData 반환
- 청크 메싱
    - 청크 스냅샷 + 이웃 스냅샷 받아서 CpuMesh 생성
- 청크 저장
    - ChunkData 스냅샷을 직렬화 후 파일 저장
- 무거운 생태계/유체/화재 틱 실행
- 결과 수거
    - 완료된 작업을 메인 스레드가 가져갈 수 있게 큐에 쌓아둠

### 인터페이스

예시 public API:

```rust
JobSystem::new(config: JobConfig) -> JobSystem
JobSystem::submit(request: JobRequest)
JobSystem::submit_all(requests: impl IntoIterator<Item = JobRequest>)
JobSystem::drain_completed() -> Vec<JobResult>
JobSystem::shutdown()
```

내부 워커는 보통 world 모듈의 API를 호출한다.

- storage::load_chunk(…)
- generation::generate_chunk(…)
- meshing::build_chunk_mesh(…)

### 의존성

- world
- simulation
- config
- logging

NOT:

- platform
- renderer
- app
- ecs

### 불변식

1. jobs는 월드 상태를 직접 들고 있지 않는다
2. 요청 입력은 가능하면 immutable snapshot 기반으로 처리한다
3. simulation job도 immutable snapshot 기반으로 처리한다
4. 작업 결과는 메인 쪽에서 명시적으로 수거되기 전까지 완료 큐에 보존된다
5. sim result는 명시적으로 수거되기 전까지 완료 큐에 보존된다
6. 같은 청크에 대한 중복 작업 시 앞에서부터 coalesce(병합)한다. 