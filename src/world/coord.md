# coord

## 역할

- world 내부에서 사용하는 블록/청크 좌표계를 정의한다.
- 월드 좌표와 청크 내부 좌표 사이의 결정적 변환 규칙을 소유한다.

## 책임

- `ChunkCoord`, `LocalBlockCoord`, `WorldBlockCoord` 정의
- 고정 청크 크기 기준 좌표 분해 규칙 정의
- world <-> chunk/local 변환 규칙 정의
- 경계 이웃 청크 판별에 필요한 좌표 규칙 정의

## 비책임

- loaded chunk map 소유
- 블록 저장
- 월드 수정
- renderer용 부동소수 좌표 변환

## 소유 데이터

- `ChunkCoord`
- `LocalBlockCoord`
- `WorldBlockCoord`
- `CHUNK_EDGE`
- `CHUNK_VOLUME`

## 입력

- integer world block position
- chunk-relative local position
- 이웃 청크/블록 질의용 오프셋

## 출력

- `(ChunkCoord, LocalBlockCoord)` 분해 결과
- `WorldBlockCoord` 재조합 결과
- 청크 경계 기반 이웃 참조 키

## 변환 규칙

1. world 좌표를 청크로 나눌 때는 고정 `CHUNK_EDGE` 기준의 결정적 규칙을 사용한다.
2. local 좌표는 항상 `0..CHUNK_EDGE` 범위로 정규화된다.
3. `ChunkCoord + LocalBlockCoord -> WorldBlockCoord`는 역변환 가능해야 한다.
4. 음수 world 좌표도 같은 규칙으로 안정적으로 분해되어야 한다.

## 공개 인터페이스

```rust
world_to_chunk_local(pos: WorldBlockCoord) -> (ChunkCoord, LocalBlockCoord)
chunk_local_to_world(chunk: ChunkCoord, local: LocalBlockCoord) -> WorldBlockCoord
is_local_in_bounds(local: LocalBlockCoord) -> bool
```

## 불변식

- 같은 `WorldBlockCoord`는 항상 같은 `(ChunkCoord, LocalBlockCoord)`로 분해된다.
- `LocalBlockCoord`는 청크 범위를 벗어나지 않는다.
- 좌표 변환은 부동소수 반올림에 의존하지 않는다.
- 청크 경계 규칙은 query/edit/meshing에서 동일하게 사용된다.

## 관련 모듈

- `chunk.md`
- `core.md`
- `edit.md`
- `query.md`
- `meshing.md`

## 메모

- 좌표계 규칙은 world 내부의 가장 낮은 수준 계약이므로, 다른 leaf가 자체 규칙을 만들면 안 된다.
- 현재 구현의 `CHUNK_EDGE`는 `32`이고, 따라서 `CHUNK_VOLUME`은 `32 * 32 * 32 = 32768`이다.
- 목표 block 스케일을 `0.5m`로 보면 청크 한 변은 개념적으로 `16m` 범위를 담당한다.
