# surface

## 역할

- renderer surface/backend 초기화와 resize를 담당한다.

## 책임

- `RenderSurfaceTarget` abstraction
- stub target과 live window target 모두 지원
- `wgpu::Instance` / `Surface` / `Adapter` / `Device` / `Queue` 생성
- depth texture 생성과 resize 재생성
- surface configure / reconfigure
- renderer bootstrap 시점과 `resumed()` 이후 attach 시점 연결

## 비책임

- frame DTO 생성
- draw call encode
- gameplay state 해석

## 공개 인터페이스

```rust
Renderer::new(target: &impl RenderSurfaceTarget, config: RenderConfig) -> Result<Renderer, RenderInitError>
Renderer::attach_window_surface(window: Arc<Window>) -> Result<(), RenderInitError>
Renderer::resize(width: u32, height: u32) -> Result<(), RenderSurfaceError>
```

## 상태 전이 규칙

- stub target으로 만들면 `RendererBackend`는 비어 있을 수 있다.
- live window가 attach되면 real `wgpu` backend를 생성한다.
- resize 시 CPU-side `SurfaceState`, camera projection cache, live surface config를 함께 갱신한다.

## 불변식

- surface 크기가 0이면 configured 상태가 아니다.
- live backend가 있을 때만 실제 surface configure/present가 일어난다.

## 관련 모듈

- `state.rs`
- `camera.rs`
- `frame.rs`

## 메모

- 현재 구현은 player cube fill pipeline, edge overlay pipeline, camera uniform buffer를 backend 생성 시 함께 준비한다.
- depth texture도 backend 생성/resize 시 같이 준비한다.
