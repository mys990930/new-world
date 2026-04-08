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
- request identity / coalesce key index
- worker assignment 추적 정보
- shutdown 중 새 요청 차단 상태

## 입력

- `submit()`으로 들어온 `JobRequest`
- worker가 보고한 completion/failure 이벤트
- `drain_completed()` 호출
- shutdown signal

## 출력

- worker에 할당할 다음 실행 대상
- 메인 스레드가 수거할 `JobResult` 목록
- queue pressure / saturation 상태

## 상태 전이 규칙

1. 새 request는 coalescing 검사를 거친 뒤 `pending`으로 들어간다.
2. worker가 유휴 상태가 되면 `pending`에서 하나를 꺼내 `running`으로 옮긴다.
3. worker completion이 오면 해당 request를 `running`에서 제거하고 `completed`에 넣는다.
4. `drain_completed()`는 현재 보관 중인 result를 고정된 순서로 반환한다.
5. shutdown이 시작되면 정책에 따라 새 request를 거부하거나 보류한다.

## 불변식

- 하나의 request는 같은 시점에 `pending`, `running`, `completed` 중 정확히 하나의 상태만 가진다.
- `running`에 있는 request는 대응 worker assignment를 가진다.
- coalescing은 결과 의미가 보존되는 request class에만 적용한다.
- completed result는 drain 전까지 손실되지 않는다.

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
- 향후 우선순위 큐가 필요해져도 `pending -> running -> completed`의 상태 모델은 유지한다.
