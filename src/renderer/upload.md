# upload

## 역할

- renderer mesh/upload DTO와 vertex layout을 정의한다.

## 책임

- `ChunkCoord`
- `RenderBounds`
- `MeshVertex`
- `CpuMesh`
- `GpuChunkMesh`
- `RenderUploadRequest`
- `RenderUploadError`
- chunk mesh bookkeeping API

## 비책임

- meshing 알고리즘
- frame draw encode
- gameplay state ownership

## 불변식

- `MeshVertex`는 `#[repr(C)]` + `Pod`/`Zeroable`이며 GPU buffer write에 바로 쓸 수 있다.
- `MeshVertex::vertex_buffer_layout()`는 player cube pipeline과 chunk pipeline이 공유할 수 있는 기본 vertex layout이다.
- 현재 chunk upload 경로는 metadata/cache bookkeeping까지만 구현돼 있다.

## 관련 모듈

- `frame.rs`
- `state.rs`

## 메모

- 현재 실제 화면 가시화는 `RenderCubeInstance` 기반 dynamic cube draw가 먼저 연결돼 있다.
