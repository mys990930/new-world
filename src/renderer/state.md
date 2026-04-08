# state

## 역할

- renderer-owned runtime state를 정의한다.

## 책임

- `Renderer` 최상위 상태 보관
- CPU-side render world bookkeeping
- optional live backend 보관
- 마지막 frame 통계와 frame index 유지

## 소유 데이터

- `RenderConfig`
- `SurfaceState`
- `PipelineSet`
- `RenderWorld`
- `CameraGpuState`
- `Option<RendererBackend>`
- `RenderStats`
- `frame_index`

## 비책임

- window 생성
- render DTO 생성
- gameplay state 소유

## 불변식

- `backend == None`일 때도 renderer는 유효한 stub 상태일 수 있다.
- live GPU resource는 `RendererBackend` 안에만 존재한다.
- `RenderWorld`는 renderer cache/meta state이지 world source of truth가 아니다.

## 관련 모듈

- `surface.rs`
- `frame.rs`
- `camera.rs`
- `upload.rs`

## 메모

- 현재 `RendererBackend`는 `wgpu::Surface`, `Device`, `Queue`, camera uniform buffer/bind group, cube pipeline을 가진다.
