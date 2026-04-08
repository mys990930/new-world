# generation

## 역할

- 월드 seed와 좌표를 바탕으로 새 청크의 원본 데이터를 계산한다.
- 절차 생성 결과를 `ChunkData`로 표현하는 계약을 정의한다.

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

## 출력

- 새 `ChunkData`

## 처리 흐름

1. `WorldMeta.seed`와 청크 좌표를 바탕으로 생성 입력을 만든다.
2. 현재 최소 구현은 청크의 world-space `y = 0` layer만 채우는 flat plane 규칙을 사용한다.
3. 각 로컬 블록 상태를 계산해 `ChunkData`에 채운다.
4. 완성된 청크를 반환한다.

## 공개 인터페이스

```rust
generation::generate_chunk(
    coord: ChunkCoord,
    meta: &WorldMeta,
) -> ChunkData
```

## 불변식

- 같은 `(seed, generator_version, coord)` 입력이면 같은 `ChunkData`가 나와야 한다.
- 생성 결과는 청크 크기와 좌표 규칙을 위반하면 안 된다.
- generation은 loaded world state를 직접 mutate하지 않는다.
- 생성 결과는 renderer용 메쉬가 아니라 원본 월드 데이터다.

## 관련 모듈

- `meta.md`
- `coord.md`
- `chunk.md`
- `core.md`
- `jobs`

## 메모

- 현재 최소 구현은 `BlockRegistry` 없이 `BlockId` 기반 규칙만 사용한다.
- 첫 vertical slice용으로 `world y = 0`에 grass block plane 한 층만 생성한다.
