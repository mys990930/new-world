# result

## 역할

- `JobResult` variant와 완료/실패 payload 경계를 정의한다.
- worker 실행 결과를 메인 스레드가 명시적으로 수거할 수 있는 형태로 고정한다.

## 소유 데이터

### JobResult
- `ChunkLoaded(coord, chunk)`
- `ChunkGenerated(coord, chunk)`
- `ChunkMeshBuilt(coord, mesh)`
- `ChunkSaved(coord)`
- `SimulationStepped(subsystem, tick, result)`
- `JobFailed(request, error)`

### Result envelope metadata
- request correlation 정보
- 완료 시각 또는 완료 순서 정보

## 입력

- worker 실행 성공 결과
- worker 실행 실패 정보
- request identity / correlation 정보

## 출력

- 메인 스레드가 drain할 completed result payload
- ECS/world/app이 후속 처리를 결정할 수 있는 success/failure 결과

## 상태 전이 규칙

- worker는 실행이 끝나면 success 또는 failure 중 하나의 result를 정확히 한 번만 방출한다.
- result는 completed queue에 들어간 뒤, drain되기 전까지 immutable로 보존된다.
- drain 후의 적용 순서는 completed queue의 deterministic order를 따른다.

## 불변식

- `JobResult`는 live world state를 직접 수정하지 않는다.
- failure도 request 식별이 가능해야 후속 재시도/정리 정책을 결정할 수 있다.
- result payload는 상위 계층이 world/ECS/renderer 후속 작업을 결정하기에 충분한 정보만 담는다.

## 비책임

- 결과 해석과 ECS 반영
- world source of truth 직접 수정
- renderer 업로드 실행

## 관련 모듈

- `queue.md`
- `worker.md`
- `../ecs/jobs.md`

## 메모

- `JobFailed`의 에러 세분화 수준은 추후 구현에서 조정 가능하지만, 최소한 재시도 가능 여부를 구분할 수 있어야 한다.
