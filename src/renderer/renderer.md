## renderer

### 역할

- GPU / surface / draw / present 경계를 담당하는 렌더링 모듈
- app bridge가 만든 render-ready DTO를 GPU 명령으로 실행한다

### 책임

- renderer bootstrap에 필요한 GPU context 생성
- surface configure / resize 처리
- render pipeline / shader / GPU camera state 관리
- CPU mesh -> GPU buffer upload / replace / remove
- visible render item draw call 기록
- submit / present 및 recoverable render error 전달

### 비책임

- meshing 알고리즘
- visible chunk 계산
- world source of truth 소유
- gameplay input 해석
- main loop orchestration

### 데이터

- `RenderConfig`
- `Renderer`
- `SurfaceState`
- `PipelineSet`
- `RenderWorld`
- `GpuChunkMesh`
- `CameraGpuState`
- `RenderStats`

### 유스케이스

- 초기화
  - surface / pipeline / camera GPU state 준비
- resize
  - surface config, projection 관련 상태 갱신
- mesh upload / 제거
  - `RenderUploadRequest`를 받아 GPU mesh cache 갱신
- 프레임 렌더
  - `RenderFrameInput`을 받아 draw / submit / present 수행

### 공개 인터페이스

```rust
Renderer::new(target: &impl RenderSurfaceTarget, config: RenderConfig) -> Result<Renderer, RenderInitError>
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

- `wgpu`
- platform의 window / surface 생성 입력
- app bridge가 만든 render DTO

NOT:

- ECS 내부 resource 구조
- world 내부 저장 구조
- app runner ownership

### 불변식

1. GPU resource는 renderer 내부에서만 생성 / 파괴한다
2. 같은 chunk coord에 대한 upload는 기존 GPU mesh를 안전하게 교체한다
3. resize 이후 surface / projection 관련 상태는 함께 갱신된다
4. renderer는 render-ready DTO만 받고 gameplay 원본 상태를 직접 소유하지 않는다

### 현재 구현 메모

- 현재 구현은 renderer skeleton + CPU-side bookkeeping 중심이다
- app bridge가 ECS camera/player 상태로부터 만든 `RenderCameraState`를 매 프레임 입력으로 받는다
- 현재 visible chunk 목록은 아직 비어 있으므로 draw call 집계도 대부분 0으로 남는다
- 실제 dynamic object draw path와 world/mesh 업로드 연결은 이후 단계에서 확장한다

### 하위 모듈 목록 및 역할

- mod.rs: public facade, re-export
- config.rs: `RenderConfig`와 renderer 정책 설정 정의
- state.rs: `Renderer`, `RenderWorld`, `GpuChunkMesh`, `RenderStats` 등 renderer-owned runtime state 정의
- surface.rs: device / queue / surface 초기화와 resize 관리
- pipeline.rs: shader / pipeline 생성과 재구성
- camera.rs: `RenderCameraState` -> `CameraGpuState` 변환과 uniform 갱신
- upload.rs: CPU mesh / render asset upload, 교체, 제거
- frame.rs: frame render pass encode, submit, present, surface error 처리
