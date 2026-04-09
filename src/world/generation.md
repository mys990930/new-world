# generation

## 역할

- 월드 seed와 좌표를 바탕으로 새 청크의 원본 데이터를 계산한다.
- 절차 생성 결과를 `ChunkData`로 표현하는 계약을 정의한다.
- atlas가 도입된 이후에는 atlas-scale 환경 해석 결과를 chunk realization 입력으로 소비하는 확장 지점을 가진다.

## 책임

- `ChunkCoord` 기준 생성 진입점 정의
- terrain/biome/block 배치 결과 계산
- 생성 결과를 `ChunkData`로 구성
- deterministic generation 보장

## 비책임

- loaded chunk map 삽입
- save/load
- async scheduling
- meshing
- gameplay rule 계산

## 입력

- `ChunkCoord`
- `WorldMeta`
- `BlockRegistry`

## 출력

- 새 `ChunkData`

## 처리 흐름

1. `WorldMeta.seed`와 청크 좌표를 바탕으로 생성 입력을 만든다.
2. 장기적으로는 atlas/biome resolver 결과를 조회해 chunk-level realization 입력을 만든다.
3. registry에서 필요한 block id를 조회한다.
4. 현재 최소 구현은 청크의 world-space `y = 0` layer 중 로컬 `(1..=5, 1..=5)` 범위만 채우는 flat patch 규칙을 사용한다.
5. 각 로컬 블록 상태를 계산해 `ChunkData`에 채운다.
6. 완성된 청크를 반환한다.

## 공개 인터페이스

```rust
generation::generate_chunk(
    coord: ChunkCoord,
    meta: &WorldMeta,
    registry: &BlockRegistry,
) -> ChunkData
```

## 불변식

- 같은 `(seed, generator_version, coord)` 입력이면 같은 `ChunkData`가 나와야 한다.
- 생성 결과는 청크 크기와 좌표 규칙을 위반하면 안 된다.
- generation은 loaded world state를 직접 mutate하지 않는다.
- 생성 결과는 renderer용 메쉬가 아니라 원본 월드 데이터다.
- generation은 block 정의를 registry에서 읽고, texture 파일을 직접 열지 않는다.

## 관련 모듈

- `meta.md`
- `coord.md`
- `chunk.md`
- `core.md`
- `registry.md`
- `atlas/atlas.md`
- `jobs`

## 메모

- 현재 최소 구현은 registry에 `"grass"` key가 있으면 `world y = 0`에 로컬 `(1,1)`부터 `(5,5)`까지의 grass block patch만 생성한다.
- 기본 manifest에서 `"grass"`가 빠지면 generator는 조용히 empty chunk를 반환한다.
- atlas prototype은 먼저 별도 debug binary에서 거시 필드와 biome preview를 검증하고, chunk realization 연결은 후속 단계에서 붙인다.
