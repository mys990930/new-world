# query

## 역할

- world 바깥 모듈이 읽기 전용으로 월드 상태를 참조할 수 있는 질의 표면을 정의한다.
- simulation / jobs / ecs / meshing이 공통으로 쓰는 snapshot/query 계약을 모은다.

## 책임

- block read
- chunk 존재 여부 확인
- chunk snapshot 제공
- region snapshot 제공
- neighbor block/chunk query 제공
- read-only block state query 제공

## 비책임

- 월드 수정
- fixed tick scheduling
- visible chunk calculation
- renderer upload

## 입력

- `WorldBlockCoord`
- `ChunkCoord`
- region description
- neighbor query description

## 출력

- `Option<BlockId>`
- `ChunkSnapshot`
- region snapshot/query 결과
- neighbor lookup 결과

## 처리 흐름

1. world 좌표 질의는 `coord` 규칙으로 `(ChunkCoord, LocalBlockCoord)`로 분해한다.
2. loaded chunk map에서 대상 청크를 찾는다.
3. 청크 내부 조회 또는 snapshot 생성을 수행한다.
4. 청크가 없으면 명시적 absence 결과를 반환한다.

## 공개 인터페이스

```rust
WorldCore::get_block(pos: WorldBlockCoord) -> Option<BlockId>
WorldCore::snapshot_chunk(coord: ChunkCoord) -> Option<ChunkSnapshot>

WorldCore::snapshot_region(...)
WorldCore::query_neighbors(...)
WorldCore::query_block_state(...)
```

## 불변식

- query는 월드 상태를 mutate하지 않는다.
- snapshot은 jobs/simulation에 넘겨도 안전한 읽기 전용 결과여야 한다.
- 없는 청크/블록에 대한 부재 표현은 호출자에게 숨기지 않는다.
- query 경계 규칙은 `coord.md`와 일관되어야 한다.

## 관련 모듈

- `coord.md`
- `chunk.md`
- `core.md`
- `meshing.md`
- `simulation`
- `jobs`

## 메모

- high-level raycast 알고리즘 자체의 최종 소유권은 아직 열어두되, 그 알고리즘이 기대하는 block/neighborhood query surface는 world가 제공한다.
