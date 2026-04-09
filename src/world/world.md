## world

### 역할

- 게임 세계의 원본 블록/청크 데이터와 정합성 있는 질의/수정 연산을 소유한다.
- 외부 모듈이 월드 내부 표현을 직접 수정하지 않도록 공용 API와 결과 타입을 정의한다.

### 책임

- loaded chunk storage
- world metadata storage
- block/chunk read-write API
- coordinate transformation rule ownership
- snapshot/query surface provision
- edit result / dirty chunk calculation
- procedural generation result expression as `ChunkData`
- save/load serialization contract
- meshing input provision from chunk snapshot bundle
- block-grid raycast query provision

### 비책임

- visible chunk calculation
- gameplay command interpretation
- fixed tick scheduling
- async worker orchestration
- gpu buffer init / draw call

### 소유 데이터

- `WorldMeta`
- `ChunkCoord`, `LocalBlockCoord`, `WorldBlockCoord`
- `BlockId`, `BlockFace`
- `ChunkData`, `ChunkSnapshot`
- `WorldEdit`, `EditResult`
- `MeshVertex`, `CpuMesh`, `RenderBounds`
- `NeighborChunks`
- `Ray3`, `RaycastHit`
- `WorldCore`

### 공개 인터페이스

```rust
WorldCore::new(meta: WorldMeta) -> WorldCore

WorldCore::has_chunk(coord: ChunkCoord) -> bool
WorldCore::insert_chunk(coord: ChunkCoord, chunk: ChunkData)
WorldCore::remove_chunk(coord: ChunkCoord) -> Option<ChunkData>

WorldCore::get_block(pos: WorldBlockCoord) -> Option<BlockId>
WorldCore::apply_edit(edit: WorldEdit) -> EditResult

WorldCore::get_chunk(coord: ChunkCoord) -> Option<&ChunkData>
WorldCore::get_chunk_mut(coord: ChunkCoord) -> Option<&mut ChunkData>
WorldCore::snapshot_chunk(coord: ChunkCoord) -> Option<ChunkSnapshot>

WorldCore::snapshot_region(...)
WorldCore::query_neighbors(...)
WorldCore::query_block_state(...)
WorldCore::raycast_blocks(ray: Ray3, max_distance: f32) -> Option<RaycastHit>

generation::generate_chunk(
    coord: ChunkCoord,
    meta: &WorldMeta,
) -> ChunkData

storage::load_chunk(bytes: &[u8]) -> Result<ChunkData, StorageError>
storage::save_chunk(snapshot: &ChunkSnapshot) -> Result<Vec<u8>, StorageError>

meshing::build_chunk_mesh(
    center: &ChunkSnapshot,
    neighbors: NeighborChunks,
) -> CpuMesh
```

### 의존성

- save format config

NOT:

- `app`
- `ecs`
- `renderer`
- `platform`

### 불변식

1. 블록은 반드시 어떤 청크 내부에만 존재한다.
2. 월드 좌표는 항상 `ChunkCoord + LocalBlockCoord`로 결정적으로 변환 가능하다.
3. `ChunkData`는 원본 월드 데이터만 가진다.
4. 모든 블록 수정은 world API를 통해서만 일어난다.
5. 청크 크기는 고정이다.
6. 없는 청크에 대한 블록 읽기/쓰기 규칙은 명확해야 한다.
   - 읽기: `None`
   - 쓰기: 실패
7. 편집 결과는 영향받은 청크 정보를 정확히 반환해야 한다.
8. 원본 월드 데이터와 렌더용 메시는 분리된다.
9. 직렬화/역직렬화는 청크 데이터 의미를 보존해야 한다.
10. raycast는 loaded world state만 기준으로 first solid hit를 판정한다.

### 하위 모듈 목록 및 역할

- `meta.md`: `WorldMeta`와 버전/seed 계약
- `coord.md`: 월드/청크/로컬 좌표계와 변환 규칙
- `chunk.md`: `ChunkData` / `ChunkSnapshot` 구조와 청크 데이터 불변식
- `core.md`: `WorldCore` 소유 구조와 top-level API
- `edit.md`: `WorldEdit` / `EditResult` 기반 명시적 mutation 계약
- `query.md`: read-only block/chunk/region/raycast query surface
- `generation.md`: 절차 생성 결과를 `ChunkData`로 표현하는 규칙
- `storage.md`: 청크 직렬화/역직렬화와 save/load 계약
- `meshing.md`: 청크 스냅샷 기반 CPU mesh 입력 제공 계약

### 현재 구현 메모

- 현재 최소 구현은 `mod.rs + leaf.rs` 구조까지 추가되어 있다.
- generation은 `WorldMeta`와 `ChunkCoord`를 받아 `world y = 0`에서 로컬 `(1..=5, 1..=5)` 범위만 채우는 deterministic grass patch generator만 제공한다.
- meshing은 world-owned `CpuMesh` / `MeshVertex`를 만들고, renderer 타입으로의 변환은 이후 jobs/app bridge 단계에서 연결한다.
- `BlockRegistry`는 아직 도입하지 않고, 현재는 `BlockId` 자체가 최소한의 solid/face-color 규칙을 가진다.
- 현재 raycast는 voxel DDA 방식으로 loaded chunk 위의 first solid block과 hit face / hit point / travel distance를 계산한다.
