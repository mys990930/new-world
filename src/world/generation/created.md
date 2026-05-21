# created

## 역할

`created`는 graph-first generator output을 bounded created-world dump로 쓰는 world-owned helper를
소유한다.

이 경로는 app/jobs가 넘긴 `CreateWorldConfig`를 받아 하나의 bounded x/z graph-first voxel plan을
만들고, 요청된 y stack을 `ChunkData`로 voxelize한 뒤 created-world storage API로 저장한다.

---

## 책임

- `CreateWorldConfig` bounds 검증
- graph-first `GraphFirstVoxelPlan` 생성
- parallel x/z stack voxelization
- created-world chunk 저장과 manifest 작성
- `CreateWorldProgress` 보고
- default preview stack summary 선정

---

## 비책임

- UI 상태 변경
- job queue/coalescing
- live `WorldCore` mutation
- chunk mesh 생성
- renderer upload

---

## 공개 API

```rust
create_graph_first_world_to_directory_with_progress(root, config, block_registry, report_progress)
```

이 API는 world public generation/storage API만 사용해 created-world dump를 완성한다. jobs는 이 함수를
호출하고 결과를 `JobResult`로 변환한다.

---

## 불변식

1. 같은 `(seed, generator_version, bounds)`는 같은 chunk dump를 만든다.
2. 같은 x/z stack은 하나의 graph-first plan column cache를 공유한다.
3. progress는 완료된 chunk 수 기준으로 보고한다.
4. chunk/manifest byte format은 기존 created-world storage API를 따른다.
