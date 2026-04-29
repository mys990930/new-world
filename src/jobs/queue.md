# queue

## 역할

- pending/running/completed 상태로 분리된 job queue를 소유한다.
- request coalescing과 deterministic drain 규칙을 관리한다.

## 소유 데이터

### JobQueue
- `PendingRequestQueue`
- `RunningRequestTable`
- `CompletedRequestQueue`

### Queue bookkeeping
- request identity / coalesce key indexes for pending, running, and not-yet-drained completed work
- worker assignment 추적 정보
- shutdown 중 새 요청 차단 상태
- pending/running/completed diagnostic counters grouped by request kind
- coalesce keys for chunk, minimap-column, create-world-root, and atlas-area work

## 입력

- `submit()`으로 들어온 `JobRequest`
- worker가 보고한 completion/failure 이벤트
- completed drain 호출
- 제한된 completed drain 호출
- shutdown signal

## 출력

- worker에 할당할 다음 실행 대상
- 메인 스레드가 수거할 `JobResult` 목록
- queue pressure / saturation 상태
- diagnostic queue snapshots for app opt-in frame logging

## 상태 전이 규칙

1. 새 request는 pending/running/not-yet-drained completed coalescing 검사를 거친 뒤 `pending`으로 들어간다.
2. worker가 유휴 상태가 되면 `pending`에서 하나를 꺼내 `running`으로 옮긴다.
3. worker completion이 오면 해당 request를 `running`에서 제거하고 `completed`에 넣는다.
4. completed drain은 현재 보관 중인 result를 고정된 순서로 반환한다.
5. 제한 drain은 같은 순서를 유지하면서 지정된 개수까지만 반환하고 나머지는 다음 drain까지 보관한다.
6. shutdown이 시작되면 정책에 따라 새 request를 거부하거나 보류한다.

## 불변식

- 하나의 request는 같은 시점에 `pending`, `running`, `completed` 중 정확히 하나의 상태만 가진다.
- `running`에 있는 request는 대응 worker assignment를 가진다.
- intermediate progress event는 queue state transition 입력이 아니며 request를 `running`에서 제거하지 않는다.
- coalescing은 결과 의미가 보존되는 request class에만 적용한다.
- final result가 완료됐지만 아직 drain되지 않은 동안에는 같은 coalesce key의 새 request도 중복으로 간주한다.
- completed result는 drain 전까지 손실되지 않는다.
- 제한 drain 뒤에 남은 completed result도 순서를 유지한 채 보존되어야 한다.
- diagnostic snapshots must be read-only and must not change pending/running/completed state

## 비책임

- job payload 실행
- gameplay 우선순위 판단
- world/simulation API 호출

## 관련 모듈

- `request.md`
- `result.md`
- `runtime.md`
- `worker.md`

## 메모

- deterministic drain order는 구현 선택과 무관하게 외부 관찰 가능 계약으로 유지한다.
- app은 gameplay 중 제한 drain을 사용해 chunk mesh upload 같은 main-thread 적용 작업을 여러 frame으로 나눌 수 있다.
- queue snapshots classify minimap and region-classification work separately so old minimap-style bottlenecks and atlas classification stalls can be identified from logs.
- 향후 우선순위 큐가 필요해져도 `pending -> running -> completed`의 상태 모델은 유지한다.
