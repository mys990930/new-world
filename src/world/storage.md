# storage

## 역할

- 청크 데이터를 save/load 가능한 바이트 표현으로 변환하는 계약을 정의한다.
- 직렬화/역직렬화가 world data 의미를 보존하도록 규칙을 고정한다.

## 책임

- `ChunkSnapshot -> bytes` 직렬화
- `bytes -> ChunkData` 역직렬화
- 저장 포맷 버전 호환성 판단
- storage error surface 정의

## 비책임

- 파일 시스템 탐색
- job scheduling
- loaded chunk map 소유
- renderer 캐시 저장

## 입력

- `ChunkSnapshot`
- serialized chunk bytes
- save format/version metadata

## 출력

- `Vec<u8>`
- `ChunkData`
- `StorageError`

## 처리 흐름

1. save 시 snapshot을 포맷 규칙에 맞게 인코딩한다.
2. load 시 바이트 헤더와 payload를 파싱한다.
3. 포맷 호환성을 확인한다.
4. 청크 의미를 복원한 `ChunkData` 또는 명시적 오류를 반환한다.

## 공개 인터페이스

```rust
storage::load_chunk(bytes: &[u8]) -> Result<ChunkData, StorageError>
storage::save_chunk(snapshot: &ChunkSnapshot) -> Result<Vec<u8>, StorageError>
```

## 불변식

- 직렬화/역직렬화 round-trip은 청크 데이터 의미를 보존해야 한다.
- 호환되지 않는 포맷은 조용히 해석하지 말고 명시적으로 실패해야 한다.
- storage payload에는 renderer 전용 캐시나 transient gameplay state가 들어가면 안 된다.
- storage는 block key 문자열이나 texture 경로가 아니라 raw `u16` block id payload만 저장한다.
- registry에 없는 block id도 load 단계에서는 그대로 복원하고, 의미 해석은 이후 registry fallback이 담당한다.

## 관련 모듈

- `meta.md`
- `chunk.md`
- `core.md`
- `jobs`

## 메모

- 실제 파일 입출력과 저장 타이밍 정책은 storage가 아니라 jobs/app 상위 계층이 결정한다.
- 현재 최소 구현은 `NWCH` magic + version + chunk coord + fixed block payload 형식의 단순 binary 포맷을 사용한다.
- 현재 block payload는 각 블록을 little-endian `u16` id로 저장한다.
