## world

### 역할

- 게임 세계의 원본 데이터와 그 데이터에 대한 정합성 있는 연산 담당

### 책임

- chunk original data storage
- block i/o
- coordinate transformation
- world metadata storage
- chunk insert/delete(unlikely)
- chunk snapshot provision
- world edit result calculation
- procedural world generation → world data expression
- meshing input data provision

### 비책임

- visible chunk calculation
- gpu buffer init / draw call

### 데이터

- WorldMeta(seed, world version, generator version, save format version)
- ChunkCoord, LocalBlockCoord, WorldBlockCoord
- BlockId
- ChunkData(?light/raw metadata)
- WorldCore(loaded chunk map, world meta, access API)
- ?BlockRegistry

### 유스케이스

- 특정 좌표의 블록 읽기
    - 월드 좌표를 받아서 해당 블록 id 반환
- 특정 좌표의 블록 수정
    - 월드 좌표의 블록을 다른 블록으로 변경
    - 수정 결과와 영향 범위 반환
- 청크 삽입
    - 로드/생성 완료된 청크를 월드에 반영??
- 청크 제거
    - 멀어진 청크를 월드에서 제거
    - 필요하면 저장 전 스냅샷 추출
- 청크 존재 여부/스냅샷 조회
    - ECS나 jobs가 현재 청크 상태를 참조할 수 있어야 함
- 청크 생성
    - seed와 좌표 기준으로 새 ChunkData 생성
- 청크 직렬화/역직렬화
    - 파일 저장/로드를 위한 바이트 변환
- 메싱 입력 제공
    - 특정 청크와 주변 청크를 바탕으로 CPU mesh 생성에 필요한 데이터 제공
- 레이캐스트/질의 보조
    - 블록 충돌용 질의
    - 블록 존재 여부 질의
    - 인접 블록 참조

### 인터페이스

일반적으로 **시스템 집합 + 스케줄 + 리소스 초기화 함수**를 제공하기. 시스템들은 일반 rust 함수로 정의하고 Schedule::add_systems(…)로 등록하는 형태. Schedule은 시스템과 실행 메타데이터를 담고, run(&but world)로 실행된다

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

Generator::generate_chunk(coord: ChunkCoord, meta: &WorldMeta, registry: &BlockRegistry) -> ChunkData

Storage::load_chunk(bytes: &[u8]) -> Result<ChunkData, StorageError>
Storage::save_chunk(chunk: &ChunkData) -> Result<Vec<u8>, StorageError>

Mesher::build_chunk_mesh(
    center: &ChunkData,
    neighbors: NeighborChunks,
    registry: &BlockRegistry,
) -> CpuMesh
```

### 의존성

- 가급적 낮은 레이어여야 한다. 의존성 거의 x
- block registry 정의
- biome/generation config

NOT:

- ecs
- renderer
- 기타 모든 상위모듈

### 불변식

1. 블록은 반드시 어떤 청크 내부에만 존재한다
2. 월드 좌표는 항상 ChunkCoord + LocalBlockCoord로 결정적으로 변환 가능하다
3. ChunkData는 원본 월드 데이터만 가진다
4. 모든 블록 수정은 world API를 통해서만 일어난다
    - 외부에서 내부 블록 배열을 직접 건드리지 않는다
5. 청크 크기는 고정이다
6. 없는 청크에 대한 블록 읽기/쓰기 규칙은 명확해야 한다
    - 읽기: None
    - 쓰기: 실패 
7. 편집 결과는 영향받은 청크 정보를 정확히 반환해야 한다
    - 경계 수정 시 이웃 청크 remesh 필요 여부를 빠뜨리면 안된다
        - ex. 나무 생성 시 옆 청크에 이파리 같은 것
8. 원본 월드 데이터와 렌더용 메쉬는 분리된다
9. 직렬화/역직렬화는 청크 데이터 의미를 보존해야 한다.