# state

## 역할

- renderer가 소유하는 런타임 상태와 GPU cache 구조를 정의한다
- 여러 하위 모듈이 공유하는 renderer-owned data shape를 한 곳에 모은다

## 소유 데이터

### Renderer
- surface state
- pipeline set
- render world
- camera gpu state
- config
- optional frame stats
- frame index

### SurfaceState
- surface snapshot
- configured / minimized state
- resize generation
- present generation

### RenderWorld
- chunk coord -> `GpuChunkMesh` map
- uploaded / removed counters
- mesh generation counter

### GpuChunkMesh
- vertex count
- index count
- triangle count
- optional bounds
- upload generation

### CameraGpuState
- view matrix
- projection matrix
- view_projection matrix
- cached aspect ratio
- last uploaded frame

### RenderStats
- draw_call_count
- submitted_chunk_count
- uploaded_mesh_count
- optional gpu / frame timing stats

## 입력

- surface.rs가 생성한 GPU context
- pipeline.rs가 생성한 pipeline handle
- upload.rs가 적용한 mesh upload 결과
- frame.rs가 갱신한 frame stats

## 출력

- renderer 내부 단계가 공통으로 사용하는 runtime state
- app이 참고할 수 있는 optional render stats

## 상태 전이 규칙

- bootstrap 이후 renderer 관련 상태는 `Renderer` 내부에 모인다
- resize 시 `SurfaceState`와 projection 관련 cache가 갱신된다
- upload / remove 시 `RenderWorld`의 GPU mesh map이 갱신된다
- render 완료 후 `RenderStats`가 최신 프레임 값으로 갱신될 수 있다

## 불변식

- renderer state는 renderer 하위 모듈만 변경한다
- `RenderWorld`는 world source of truth가 아니라 GPU cache다
- `GpuChunkMesh`는 coord key와 1:1로 대응하는 현재 렌더 표현만 가진다
- `SurfaceState`의 surface config와 depth texture 상태는 항상 호환되어야 한다

## 비책임

- meshing 알고리즘
- visible chunk 계산
- gameplay camera 상태 소유
- main loop 제어

## 관련 모듈

- surface.rs가 `SurfaceState`를 생성 / 갱신
- pipeline.rs가 `Renderer` 내부 pipeline set을 채운다
- camera.rs가 `CameraGpuState`를 갱신
- upload.rs가 `RenderWorld`를 갱신
- frame.rs가 `RenderStats`를 갱신

## 메모

- material system이 커지면 `RenderWorld`를 `mesh_cache.rs`, `material_cache.rs`로 다시 쪼갤 수 있다
- 현재 단계에서는 chunk mesh 렌더링 중심의 최소 상태만 먼저 가정한다
- 현재 1차 구현에서 `SurfaceState`와 `GpuChunkMesh`는 실제 GPU 핸들이 아니라 backend-less bookkeeping data를 저장한다
