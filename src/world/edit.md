# edit

## 역할

- world mutation을 명시적 `WorldEdit`와 `EditResult` 계약으로 표현한다.
- 블록 수정의 성공/실패와 영향 범위를 world 경계 안에서 계산한다.

## 책임

- `WorldEdit` 형태 정의
- 편집 대상 좌표/청크 검증
- block/chunk mutation 적용
- dirty chunk / remesh 영향 범위 계산
- `EditResult` 반환

## 비책임

- gameplay command 해석
- fixed tick scheduling
- renderer upload 요청 수행
- save job 제출

## 입력

- `WorldEdit`
- 현재 loaded chunk 상태
- 좌표 변환 규칙

## 출력

- `EditResult`
  - 성공/실패 여부
  - 변경된 청크 목록
  - remesh가 필요한 청크 목록
  - 후속 저장/동기화 판단에 필요한 영향 정보

## 처리 흐름

1. `WorldEdit`를 대상 블록/청크 단위로 정규화한다.
2. 필요한 청크가 모두 존재하는지 확인한다.
3. `ChunkData`를 수정한다.
4. 경계 변경 여부를 바탕으로 영향 청크를 계산한다.
5. `EditResult`를 반환한다.

## 공개 인터페이스

```rust
WorldCore::apply_edit(edit: WorldEdit) -> EditResult
```

## 불변식

- 모든 블록 수정은 explicit edit를 통해서만 일어난다.
- 없는 청크에 대한 쓰기는 명시적으로 실패한다.
- 경계 수정 시 이웃 청크 remesh 필요 여부를 빠뜨리면 안 된다.
- 부분 성공 허용 여부는 `EditResult`에서 명시적으로 표현되어야 한다.

## 관련 모듈

- `core.md`
- `coord.md`
- `chunk.md`
- `query.md`
- `simulation`
- `ecs`

## 메모

- simulation은 world를 직접 mutate하지 않고 `WorldEdit`를 계산해 돌려주는 쪽을 기본 경로로 본다.
