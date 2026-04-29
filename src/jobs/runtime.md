# runtime

## 역할

- `JobSystem`을 통해 jobs 모듈의 top-level runtime 진입점과 public API를 제공한다.
- config, queue, worker context를 조립하고 외부 제출/수거 표면을 노출한다.

## 책임

- `JobConfig`를 받아 `JobSystem` 생성
- worker context 초기화
- request 제출 API 제공
- 완료 결과 drain API 제공
- shutdown sequence 진입

## 비책임

- 어떤 request를 만들지 결정
- result의 gameplay 의미 해석
- 개별 job type 구현

## 소유 데이터

### JobSystem
- `config`
- `queue`
- `worker_context`
- intermediate progress result buffer

## 공개 인터페이스

```rust
JobSystem::new(config: JobConfig) -> JobSystem
JobSystem::submit(request: JobRequest) -> Result<JobEnqueueOutcome, JobSubmitError>
JobSystem::submit_all(requests: impl IntoIterator<Item = JobRequest>) -> Result<(), JobSubmitError>
JobSystem::drain_completed() -> Vec<JobResult>
JobSystem::drain_completed_limit(max_results: usize) -> Vec<JobResult>
JobSystem::diagnostic_snapshot() -> JobSystemSnapshot
JobSystem::shutdown()
```

## 입력

- `JobConfig`
- 상위 모듈이 생성한 `JobRequest`
- worker completion 이벤트
- worker progress 이벤트

## 출력

- 완료된 final `JobResult` drain surface
- frame budget에 맞춘 제한 drain surface
- long-running job progress `JobResult` drain surface
- read-only job-system diagnostic snapshots
- shutdown 완료 상태

## 상태 전이 규칙

- 초기화 시 config에 따라 worker를 준비하고 queue를 빈 상태로 시작한다.
- `submit()`은 request를 queue 경계로 넘긴다.
- `drain_completed()`는 intermediate progress buffer와 queue가 보관한 final result를 가져온다.
- `drain_completed_limit(...)`는 같은 drain 순서를 유지하면서 호출자가 지정한 최대 개수까지만 가져온다.
- `diagnostic_snapshot()`은 queue pressure, worker availability, and request-kind counts를 읽기 전용으로 반환한다.
- `shutdown()`은 새 요청 수용 중단과 worker 종료 수순을 시작한다.

## 불변식

- public API는 live world borrow를 외부에 요구하지 않는다.
- `JobSystem`은 queue와 worker 수명을 함께 소유한다.
- progress report는 queue의 running state를 완료시키지 않는다.
- 제한 drain 뒤에 남은 progress/final result는 다음 drain까지 보존되어야 한다.
- diagnostic snapshot reads must not collect worker messages or dispatch new work.
- shutdown 이후에는 request 수용 정책이 명확해야 한다.

## 관련 모듈

- `config.md`
- `queue.md`
- `worker.md`
- `request.md`
- `result.md`

## 메모

- 현재 구현은 config에 따라 worker thread를 만들고, `submit()`/`drain_completed()` 시점에 결과 수거와 pending dispatch를 함께 진행한다.
- `submit()`는 새 요청이 실제로 enqueue됐는지, 기존 요청과 coalesced됐는지 `JobEnqueueOutcome`으로 알려준다.
- `drain_completed()`는 app이 같은 surface에서 progress와 final result를 처리할 수 있도록 `JobResult` 목록을 유지하되, final ordering 규칙은 queue가 계속 소유한다.
- `drain_completed_limit(...)`는 gameplay frame에서 world insertion, minimap update, renderer upload를 여러 frame으로 나누기 위한 surface다.
- `diagnostic_snapshot()` is used by app opt-in frame logging to show pending/running/completed counts split across create/load/generate/mesh/minimap/region-classification work.
- current diagnostics log job-system startup worker count and shutdown requests.
