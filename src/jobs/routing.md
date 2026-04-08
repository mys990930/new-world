# routing

## 역할

- `JobRequest` variant를 실제 `world`/`simulation` 작업 함수로 연결한다.
- job type별 입력/출력 매핑 규칙을 한 곳에 모은다.

## 책임

- `LoadChunk` -> storage load 경로 연결
- `GenerateChunk` -> procedural generation 경로 연결
- `BuildChunkMesh` -> meshing 경로 연결
- `SaveChunk` -> storage save 경로 연결
- simulation tick 계열 요청 -> simulation 실행 경로 연결
- 성공/실패를 `JobResult`로 변환

## 비책임

- queue 상태 관리
- worker lifecycle 관리
- gameplay 의미 해석
- live world source of truth 직접 수정

## 입력

- `JobRequest`
- `world` 공용 API와 데이터 타입
- `simulation` 공용 API와 데이터 타입

## 출력

- `JobResult`
- type별 실행 에러

## 처리 흐름

1. request variant를 판별한다.
2. 해당 variant에 맞는 `world` 또는 `simulation` API를 호출한다.
3. 성공 값을 대응 `JobResult` variant로 감싼다.
4. 실패 시 `JobFailed(...)`로 변환한다.

## 상태 전이 규칙

- routing은 request payload를 해석하지만 queue 상태를 직접 바꾸지 않는다.
- 외부 API 호출은 request에 포함된 immutable payload만 사용한다.
- simulation job은 fixed tick 소유권이 아니라 해당 tick 계산만 수행한다.

## 불변식

- routing은 `world` 내부 표현에 직접 결합하지 않고 문서화된 공용 API만 사용한다.
- meshing/generation/storage/simulation 호출 결과는 `JobResult` 경계로만 외부에 노출된다.
- 실패도 success와 동일한 correlation 경계를 따라 반환된다.

## 관련 모듈

- `request.md`
- `result.md`
- `worker.md`
- `../world/world.md`
- `../simulation/simulation.md`

## 메모

- 현재 문서 단계에서는 `storage::load_chunk(...)`, `generation::generate_chunk(...)`, `meshing::build_chunk_mesh(...)` 같은 호출 표면만 고정하고, 실제 함수 시그니처 세부는 각 모듈 구현 시점에 확정한다.
