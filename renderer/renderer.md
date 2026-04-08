## renderer

### 역할

- 렌더링에 필요한 GPU 리소스 관리 및 프레임 그리기

### 책임

- GPU device/queue/surface management
- swapchain/surface resize processing
- CPU mesh → GPU buffer upload
- render pipeline/shader management
- camera/uniform management
- draw call
- present

### 비책임

- meshing algorithm
- visible chunk calculation

### 데이터

#### Renderer

- device
- queue
- surface
- surface config
- pipelines
- bind groups
- depth texture 등

#### GpuChunkMesh

- vertext buffer
- index buffer
- index count

#### RenderWorld

- 청크 별 GPU mesh map
- material/texture handles
- frame temp data

#### CameraGpuState

- view/projection uniform
- camera buffer

### 유스케이스

- 초기화
    - window/surface 받아서 renderer 생성
    - pipeline/shader 준비
- resize
    - surface 재설정
    - depth texture 재생성
    - viewport/projection 갱신
- mesh 업로드
    - CpuMesh를 받아 vertext/index buffer 생성
    - coord 기준으로 기존 GPU mesh 교체
- mesh 제거
    - 멀어진 청크의 GPU 리소스 제거
- 프레임 렌더
    - visible chunk 목록을 받아서 draw
    - entity/UI/Debug overlay 등 추가 draw 기능

### 인터페이스

```rust
Renderer::new(window_handle: &WindowHandle, config: RenderConfig) -> Renderer
Renderer::resize(width: u32, height: u32)

Renderer::upload_chunk_mesh(coord: ChunkCoord, mesh: CpuMesh)
Renderer::remove_chunk_mesh(coord: ChunkCoord)

Renderer::render(frame_input: RenderFrameInput) -> Result<(), RenderError>

struct RenderFrameInput<'a> {
    camera: &'a CameraState,
    visible_chunks: &'a [ChunkCoord],
    world: &'a WorldCore, // 필요 최소만
}
```

### 의존성

- wgpu
- platform의 window handle
- world::CpuMesh 타입
- shader helper

NOT:

- ecs
- jobs
- app
    - renderer는 ecs의 리소스 구조를 직접 알 필요 없다. app이 camera/visible list를 추출해서 넘기면 됨

### 불변식

1. GPU 리소스는 renderer에서만 생성/파괴한다
2. 같은 청크 coord에 대해 업로드 시 기존 GPU mesh를 일관되게 교체 또는 덮어쓴다.
3. resize 이후 surface/pipeline 관련 상태는 항상 일관되어야 한다
4. CPU mesh ↔ GPU mesh 는 분리된다