# config

## 역할

- jobs runtime이 사용하는 실행 정책을 typed config로 정의한다.
- worker 개수, queue 압력, shutdown 동작 같은 운영 정책을 코드 흐름과 분리한다.

## 소유 데이터

### JobConfig
- `worker_count`
- `max_pending_requests`
- `max_completed_results`
- `coalesce_policy`
- `shutdown_policy`

## 입력

- 하드코딩 기본값
- 향후 config file / CLI / 환경변수 결과
- 플랫폼별 thread/task runtime 제약

## 출력

- `JobSystem::new(...)`에 전달되는 typed config
- queue/worker가 참조하는 운영 정책

## 상태 전이 규칙

- bootstrap 이후 config는 immutable로 취급한다.
- 런타임 중 변하는 queue 길이, in-flight 상태는 config가 아니라 runtime state에 둔다.
- `worker_count`와 queue limit은 worker spawn 전에 확정한다.

## 불변식

- `worker_count`는 `1` 이상이어야 한다.
- queue limit이 있다면 `pending`과 `completed`의 보존 정책이 명확해야 한다.
- coalescing은 config로 켜더라도 모든 job type에 무조건 적용하지 않는다.

## 비책임

- 실제 worker spawn 코드
- gameplay 차원의 job 우선순위 판단
- result 해석과 ECS 반영

## 관련 모듈

- `runtime.md`
- `queue.md`
- `worker.md`

## 메모

- 현재 단계에서는 `JobConfig`가 최소한 worker 수와 queue 정책을 담는다는 계약만 고정한다.
- 추후 profiling 결과에 따라 job class별 별도 제한값을 추가할 수 있다.
