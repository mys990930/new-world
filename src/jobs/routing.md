# routing

## 역할

- `JobRequest` variant를 실제 `world` 작업 함수로 연결한다.
- job type별 입력/출력 매핑 규칙을 한 곳에 모은다.

## 책임

- `GenerateChunk` -> procedural generation 경로 연결
- `BuildChunkMesh` -> meshing 경로 연결
- 성공 값을 `JobResult`로 변환

## 비책임

- queue 상태 관리
- worker lifecycle 관리
- gameplay 의미 해석
- live world source of truth 직접 수정

## 입력

- `JobRequest`
- `world` 공용 API와 데이터 타입

## 출력

- `JobResult`

## 처리 흐름

1. request variant를 판별한다.
2. 해당 variant에 맞는 `world` API를 호출한다.
3. 성공 값을 대응 `JobResult` variant로 감싼다.

## 상태 전이 규칙

- routing은 request payload를 해석하지만 queue 상태를 직접 바꾸지 않는다.
- 외부 API 호출은 request에 포함된 immutable payload만 사용한다.

## 불변식

- routing은 `world` 내부 표현에 직접 결합하지 않고 문서화된 공용 API만 사용한다.
- meshing/generation 호출 결과는 `JobResult` 경계로만 외부에 노출된다.

## 관련 모듈

- `request.md`
- `result.md`
- `worker.md`
- `../world/world.md`

## 메모

- 현재 최소 구현은 `generation::generate_chunk(...)`와 `meshing::build_chunk_mesh(...)` 두 경로만 실제로 연결한다.
- load/save/simulation routing은 다음 단계에서 leaf request/result variant와 함께 확장한다.
