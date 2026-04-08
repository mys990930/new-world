# request

## 역할

- `JobRequest` variant와 worker-safe 입력 payload 경계를 정의한다.
- 상위 모듈이 jobs에 무엇을 요청할 수 있는지 명시한다.

## 소유 데이터

### JobRequest
- `GenerateChunk { coord, meta }`
- `BuildChunkMesh { center, neighbors }`

### Request identity / coalesce key
- chunk 좌표 기반 dedupe 식별자

## 입력

- `ChunkCoord`
- immutable `ChunkSnapshot`
- `NeighborChunks`
- `WorldMeta`

## 출력

- worker가 직접 실행할 수 있는 owned request payload
- queue가 사용할 request identity / coalesce key

## 상태 전이 규칙

- request는 상위 계층이 live world borrow 대신 snapshot/value payload로 생성한다.
- queue 진입 전 coalesce key를 계산할 수 있어야 한다.
- worker가 실행을 시작하면 request payload는 immutable로 취급한다.

## 불변식

- `JobRequest`는 worker thread/task 경계 너머로 안전하게 전달 가능해야 한다.
- request payload는 live world 내부 배열에 대한 참조를 들고 있지 않는다.
- 동일 request type이라도 merge가 안전하지 않으면 coalescing 대상이 아니다.

## 비책임

- request 실행
- 완료 결과 적용
- queue 스케줄링 정책 결정

## 관련 모듈

- `queue.md`
- `routing.md`
- `../ecs/jobs.md`
- `../world/world.md`

## 메모

- 현재 최소 구현은 block plane vertical slice에 필요한 `GenerateChunk`와 `BuildChunkMesh`만 지원한다.
- 중복 coalescing은 두 request 모두 chunk 좌표 기준으로만 적용한다.
