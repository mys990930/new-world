# core

## 역할

- loaded world state와 top-level world API를 소유한다.
- 외부 모듈이 world 내부 표현 대신 `WorldCore`를 통해 상호작용하도록 경계를 만든다.

## 책임

- `WorldCore` 정의
- `WorldMeta` 보관
- loaded chunk map 소유
- chunk insert/remove 관리
- block/chunk read API 제공
- edit/query/generation/storage/meshing 하위 계약을 묶는 진입점 제공

## 비책임

- fixed tick scheduling
- job queue orchestration
- gameplay command 의미 해석
- renderer draw/upload
- 절차 생성 실행 정책

## 소유 데이터

### WorldCore

- `meta`
- loaded chunk map keyed by `ChunkCoord`
- public access policy for query/edit operations

## 입력

- `WorldMeta`
- load/generation 결과로 생성된 `ChunkData`
- block/chunk query 요청
- explicit `WorldEdit`

## 출력

- chunk 존재 여부/참조 결과
- 제거된 `ChunkData`
- `EditResult`
- snapshot/query 결과

## 처리 흐름

1. 외부 요청은 `WorldCore`로 진입한다.
2. 필요하면 `coord` 규칙으로 좌표를 분해한다.
3. loaded chunk map에서 대상 청크를 찾는다.
4. `chunk`, `edit`, `query` 하위 계약을 사용해 결과를 계산한다.
5. 호출자에게 명시적 결과를 반환한다.

## 공개 인터페이스

```rust
WorldCore::new(meta: WorldMeta) -> WorldCore

WorldCore::has_chunk(coord: ChunkCoord) -> bool
WorldCore::insert_chunk(coord: ChunkCoord, chunk: ChunkData)
WorldCore::remove_chunk(coord: ChunkCoord) -> Option<ChunkData>

WorldCore::get_block(pos: WorldBlockCoord) -> Option<BlockId>
WorldCore::get_chunk(coord: ChunkCoord) -> Option<&ChunkData>
WorldCore::get_chunk_mut(coord: ChunkCoord) -> Option<&mut ChunkData>
WorldCore::snapshot_chunk(coord: ChunkCoord) -> Option<ChunkSnapshot>

WorldCore::apply_edit(edit: WorldEdit) -> EditResult
```

## 불변식

- loaded chunk map의 source of truth owner는 `WorldCore`다.
- 외부 모듈은 내부 블록 배열을 직접 건드리지 않는다.
- 없는 청크에 대한 읽기/쓰기 규칙은 world 차원에서 일관되게 유지된다.
- `WorldMeta`는 chunk 개별 데이터보다 상위 레벨에서 유지된다.

## 관련 모듈

- `meta.md`
- `coord.md`
- `chunk.md`
- `edit.md`
- `query.md`
- `generation.md`
- `storage.md`

## 메모

- generation / storage / meshing은 `WorldCore`가 소유하는 loaded state에 직접 붙기보다, 명시적 입력/출력 타입으로 연결된다.
