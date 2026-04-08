# chunk

## 역할

- 청크 단위 원본 월드 데이터 구조를 정의한다.
- mutable `ChunkData`와 immutable `ChunkSnapshot`의 의미를 분리한다.

## 책임

- 고정 크기 블록 저장 구조 정의
- 청크 내부 block read/write 규칙 정의
- 청크 스냅샷 표현 정의
- 원본 월드 데이터 전용 필드 범위 정의

## 비책임

- world 좌표 분해
- loaded chunk map 소유
- edit 영향 범위 계산
- 직렬화 바이트 포맷 정의
- meshing 알고리즘 실행
- renderer 캐시 보관

## 소유 데이터

### ChunkData

- block storage
- optional raw metadata
- future light/raw block-state payloads

### ChunkSnapshot

- immutable block/raw metadata view or copy
- jobs / simulation / meshing 전달용 read-only payload

## 입력

- `LocalBlockCoord`
- `BlockId`
- snapshot request

## 출력

- local block read 결과
- local block write 결과
- immutable `ChunkSnapshot`

## 상태 전이 규칙

- `ChunkData` mutation은 world-owned API 안에서만 일어난다.
- `ChunkSnapshot`은 생성 시점의 청크 의미를 고정한다.
- snapshot 생성 이후의 `ChunkData` 변화는 기존 snapshot에 역으로 반영되지 않는다.

## 공개 인터페이스

```rust
ChunkData::get_block(local: LocalBlockCoord) -> Option<BlockId>
ChunkData::set_block(local: LocalBlockCoord, block: BlockId) -> Result<(), ChunkWriteError>
ChunkData::snapshot(&self) -> ChunkSnapshot
```

## 불변식

- 청크 크기는 고정이다.
- `ChunkData`는 원본 월드 데이터만 가진다.
- renderer 전용 메쉬나 transient gameplay state는 `ChunkData`에 들어가지 않는다.
- `ChunkSnapshot`은 읽기 전용이며 jobs/simulation으로 안전하게 전달 가능해야 한다.

## 관련 모듈

- `coord.md`
- `core.md`
- `edit.md`
- `query.md`
- `storage.md`
- `meshing.md`

## 메모

- light, raw metadata, block state payload는 필요해질 때 추가할 수 있지만, 여전히 "원본 월드 데이터" 범위를 벗어나면 안 된다.
