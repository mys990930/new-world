## renderer

### 역할

- GPU / surface / draw / present 경계를 담당하는 렌더링 모듈
- app/bridge가 넘긴 렌더링 입력을 GPU 명령으로 실행한다

### 책임

- renderer bootstrap에 필요한 GPU context 생성
- surface / swapchain configure 및 resize 처리
- render pipeline / shader / bind group layout 관리
- camera / frame-global uniform GPU 반영
- CPU mesh -> GPU buffer upload / replace / remove
- visible render item draw call 기록
- submit / present 및 recoverable render error 전달

### 비책임

- meshing algorithm
- visible chunk 계산
- 블록 원본 데이터 소유
- 입력 해석
- 프레임 루프 orchestration
- gameplay UI 상태 판단

### 데이터

- RenderConfig
- Renderer
- SurfaceState
- PipelineSet
- RenderWorld
- GpuChunkMesh
- CameraGpuState
- RenderStats

### 유스케이스

- 초기화
    - window / surface 생성 입력을 받아 renderer 생성
    - surface config, depth texture, pipeline, camera GPU 상태 준비
- resize
    - 새 창 크기를 반영해 surface 재설정
    - depth texture와 projection 관련 상태 갱신
- mesh 업로드 / 제거
    - `RenderUploadRequest`를 받아 GPU mesh cache를 갱신
    - 멀어진 청크나 무효화된 청크의 GPU 리소스를 제거
- 프레임 렌더
    - `RenderFrameInput`을 받아 draw / submit / present 수행
    - 필요 시 recoverable error를 app에 반환

### 인터페이스

```rust
Renderer::new(window: &PlatformWindowHandle, config: RenderConfig) -> Result<Renderer, RenderInitError>
Renderer::resize(width: u32, height: u32) -> Result<(), RenderSurfaceError>

Renderer::apply_upload(request: RenderUploadRequest) -> Result<(), RenderUploadError>
Renderer::remove_chunk_mesh(coord: ChunkCoord)

Renderer::render(frame: RenderFrameInput<'_>) -> Result<RenderStats, RenderError>

struct RenderFrameInput<'a> {
    camera: &'a RenderCameraState,
    visible_chunks: &'a [ChunkCoord],
    clear_color_override: Option<[f32; 4]>,
}
```

### 의존성

- wgpu
- platform의 window / surface 생성 입력
- app::bridge가 만든 render DTO
- shader asset / helper

NOT:

- ecs 내부 리소스 구조
- world 내부 저장 구조
- app runner / main loop 소유권

### 불변식

1. GPU 리소스는 renderer 내부에서만 생성 / 파괴한다
2. 같은 청크 coord에 대한 upload는 기존 GPU mesh를 일관되게 교체한다
3. resize 이후 surface config / depth texture / projection 관련 상태는 항상 일관되어야 한다
4. CPU mesh cache와 world source of truth는 분리된다
5. renderer는 render-ready DTO만 읽고, 도메인 원본 상태를 직접 소유하지 않는다

### 하위 모듈 목록 및 역할

- mod.rs: public facade, re-export, renderer public API entry
- config.rs: `RenderConfig`와 렌더링 정책 설정 정의
- state.rs: `Renderer`, `RenderWorld`, `GpuChunkMesh`, `RenderStats` 등 renderer-owned runtime state 정의
- surface.rs: device / queue / surface 초기화, configure, resize, depth texture 관리
- pipeline.rs: shader module, pipeline layout, render pipeline 생성 / 재구성
- camera.rs: `RenderCameraState` -> `CameraGpuState` 변환 및 uniform upload
- upload.rs: CPU mesh / render asset 업로드, 교체, 제거 경로
- frame.rs: frame render pass encode, submit, present, surface error 처리

