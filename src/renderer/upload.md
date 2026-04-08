# upload

## 역할

- CPU 쪽 렌더 자산을 GPU cache로 반영하는 업로드 경로를 담당한다

## 책임

- `RenderUploadRequest`를 해석한다
- CPU mesh를 vertex / index buffer로 업로드한다
- 같은 coord의 기존 `GpuChunkMesh`를 교체한다
- remove 요청을 처리한다
- 필요 시 old GPU resource drop / destroy 타이밍을 정리한다

## 비책임

- meshing algorithm
- dirty chunk 판단
- visible chunk 계산
- 실제 draw pass 실행
- world source data 수정

## 입력

- `RenderUploadRequest`
- chunk coord
- CPU mesh payload
- `SurfaceState`의 device / queue

## 출력

- 갱신된 `RenderWorld`
- optional upload 통계
- recoverable upload error

## 처리 흐름

1. upload request를 받는다
2. mesh payload가 유효한지 검사한다
3. GPU buffer를 생성하고 데이터를 업로드한다
4. coord key의 기존 mesh와 교체한다
5. remove 요청이면 GPU cache에서 해당 coord를 제거한다

## 불변식

- 같은 coord에 대한 교체는 renderer 내부에서 원자적으로 보이도록 처리한다
- upload 경로는 world source of truth를 수정하지 않는다
- 빈 mesh / 잘못된 payload에 대한 처리 정책은 항상 동일해야 한다

## 관련 모듈

- state.rs가 `RenderWorld`와 `GpuChunkMesh`를 소유
- surface.rs가 device / queue를 제공
- frame.rs가 upload 이후 최신 GPU cache를 사용한다
- app::bridge가 world dirty state를 `RenderUploadRequest`로 변환한다
- jobs / world가 CPU mesh 생산에 관여할 수 있다

## 메모

- 업로드량이 커지면 staging belt, batched upload, frame budget 정책을 추가할 수 있다
- texture / material 업로드가 생기면 `mesh_upload.rs`, `texture_upload.rs`로 다시 분리 가능하다
- 현재 1차 구현의 `GpuChunkMesh`는 실제 GPU buffer 핸들이 아니라, 검증된 mesh count / bounds / upload generation metadata를 담는다
