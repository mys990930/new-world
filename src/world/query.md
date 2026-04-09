# query

## 역할

- world 바깥 모듈이 읽기 전용으로 월드 상태를 참조할 수 있는 질의 표면을 정의한다.
- simulation / jobs / ecs / meshing이 공통으로 기대하는 snapshot/query 계약을 모은다.

## 책임

- block read
- chunk 존재 여부 확인
- chunk snapshot 제공
- region snapshot 제공
- neighbor block/chunk query 제공
- read-only block state query 제공
- high-level block-grid raycast 제공

## 비책임

- 월드 수정
- fixed tick scheduling
- visible chunk calculation
- renderer upload
- selection policy 결정

## 입력

- `WorldBlockCoord`
- `ChunkCoord`
- region description
- neighbor query description
- `Ray3`
- raycast 최대 거리

## 출력

- `Option<BlockId>`
- `ChunkSnapshot`
- region snapshot/query 결과
- neighbor lookup 결과
- `RaycastHit`

## 처리 흐름

1. world 좌표 질의는 `coord` 규칙으로 `(ChunkCoord, LocalBlockCoord)`로 분해한다.
2. loaded chunk map에서 대상 청크를 찾는다.
3. 청크 내부 조회, snapshot 생성, 혹은 ray stepping을 수행한다.
4. 청크가 없으면 명시적 absence 결과를 반환한다.

## 공개 인터페이스

```rust
WorldCore::get_block(pos: WorldBlockCoord) -> Option<BlockId>
WorldCore::snapshot_chunk(coord: ChunkCoord) -> Option<ChunkSnapshot>

WorldCore::snapshot_region(...)
WorldCore::query_neighbors(...)
WorldCore::query_block_state(...)
WorldCore::raycast_blocks(ray: Ray3, max_distance: f32) -> Option<RaycastHit>
```

## 불변식

- query는 world 상태를 mutate하지 않는다.
- snapshot은 jobs/simulation이 읽기 전용으로 넘겨받을 수 있는 결과여야 한다.
- 없는 청크/블록에 대한 부재 표현은 호출자에게 숨기지 않는다.
- query 경계 규칙은 `coord.md`와 일관되어야 한다.
- raycast는 loaded chunk 밖을 가로질러도 새로운 청크를 생성하지 않는다.

## 관련 모듈

- `coord.md`
- `chunk.md`
- `core.md`
- `meshing.md`
- `simulation`
- `jobs`
- `ecs`

## 메모

- 현재 raycast는 voxel DDA 기반이며, loaded world 위에서 first solid hit를 반환한다.
- 타겟 우선순위, hover 전환, 배치 프리뷰처럼 gameplay 의미가 있는 선택 규칙은 world가 아니라 ECS selection이 담당한다.
