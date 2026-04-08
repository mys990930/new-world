## renderer

### 역할

- GPU / surface / draw / present 경계를 담당하는 렌더러 모듈
- app bridge가 만든 render-ready DTO를 GPU 명령으로 실행한다

### 책임

- renderer bootstrap과 GPU context 준비
- stub surface 상태와 live window surface attach
- surface configure / resize 처리
- render pipeline / shader / GPU camera state 관리
- depth buffer 생성과 유지
- CPU render DTO -> GPU draw command 변환
- submit / present / recoverable render error 전달

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
- `CameraGpuState`
- `RenderCubeInstance`
- `RenderStats`

### 유스케이스

- 초기화
  - stub target 또는 live window target로 renderer 생성
- live surface attach
  - `resumed()` 이후 실제 window를 받아 `wgpu` backend 생성
- resize
  - surface config와 projection 관련 상태 갱신
- frame render
  - `RenderFrameInput`을 받아 camera uniform 갱신, clear, dynamic cube draw, present 수행

### 공개 인터페이스

```rust
Renderer::new(target: &impl RenderSurfaceTarget, config: RenderConfig) -> Result<Renderer, RenderInitError>
Renderer::attach_window_surface(window: Arc<Window>) -> Result<(), RenderInitError>
Renderer::resize(width: u32, height: u32) -> Result<(), RenderSurfaceError>

Renderer::apply_upload(request: RenderUploadRequest) -> Result<(), RenderUploadError>
Renderer::remove_chunk_mesh(coord: ChunkCoord)

Renderer::render(frame: RenderFrameInput<'_>) -> Result<RenderStats, RenderError>

struct RenderFrameInput<'a> {
    camera: &'a RenderCameraState,
    visible_chunks: &'a [ChunkCoord],
    cube_instances: &'a [RenderCubeInstance],
    clear_color_override: Option<[f32; 4]>,
}
```

### 의존성

- `wgpu`
- platform window / surface creation input
- app bridge가 만든 render DTO

NOT:

- ECS 내부 resource 구조
- world 내부 데이터 구조
- app runner ownership

### 불변식

1. renderer는 render-ready DTO만 받는다.
2. bootstrap 시점에는 stub renderer만 있어도 되지만, live draw는 real backend가 붙은 뒤에만 수행한다.
3. GPU resource 생성/파괴는 renderer 내부에서만 일어난다.

### 현재 구현 메모

- 현재 구현은 플레이어 큐브 1개를 그리는 최소 dynamic object path까지 연결돼 있다.
- 플레이어 큐브 path는 depth test/write를 사용해 실제 큐브 실루엣이 보이도록 한다.
- chunk upload 경로는 아직 CPU-side bookkeeping 위주이며 실제 chunk draw는 이후 단계다.

### 하위 모듈 목록 및 역할

- mod.rs: public facade, re-export
- config.rs: `RenderConfig`와 renderer 정책 설정
- state.rs: `Renderer`, `RenderWorld`, optional `RendererBackend`
- surface.rs: device / queue / surface 초기화, live surface attach, resize 관리
- pipeline.rs: pipeline metadata
- camera.rs: `RenderCameraState` -> `CameraGpuState` / `CameraUniform` 변환
- upload.rs: mesh/upload DTO와 vertex layout 정의
- frame.rs: frame render pass encode, dynamic cube draw, submit, present
