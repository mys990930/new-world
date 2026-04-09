# meshing

## 역할

- 원본 청크 스냅샷을 기반으로 렌더링 가능한 CPU mesh 입력을 계산한다.
- world 데이터에서 renderer upload DTO로 넘어가기 직전의 경계를 정의한다.

## 책임

- center + neighbor chunk snapshot bundle 정의
- block registry 기반 render/opacity/face-texture 해석
- 블록 노출면 판단에 필요한 주변 블록 조회 규칙 정의
- `CpuMesh` 생성 계약 정의
- 청크 경계 face 처리 규칙 정의

## 비책임

- visible chunk calculation
- GPU upload
- draw call 실행
- 월드 수정

## 입력

- center `ChunkSnapshot`
- neighbor chunk snapshots
- `BlockRegistry`

## 출력

- world-owned `CpuMesh`
  - `MeshVertex { position, color, normal, uv, texture_layer }`

## 처리 흐름

1. center 청크의 블록을 순회한다.
2. 각 블록의 render kind, tint, face texture, opacity를 registry에서 해석한다.
3. 각 블록의 렌더 가능 여부와 면 노출 여부를 판단한다.
4. 경계 면은 neighbor snapshot을 함께 참조해 판정한다.
5. 결과를 `CpuMesh`로 모아 반환한다.

## 공개 인터페이스

```rust
meshing::build_chunk_mesh(
    center: &ChunkSnapshot,
    neighbors: NeighborChunks,
    registry: &BlockRegistry,
) -> CpuMesh
```

## 불변식

- meshing은 입력 snapshot을 mutate하지 않는다.
- 청크 경계 face 판정은 이웃 청크 존재 여부를 올바르게 반영해야 한다.
- opaque face culling은 registry가 해석한 block opacity를 기준으로 계산해야 한다.
- meshing 결과는 renderer upload용 CPU 데이터일 뿐, GPU 리소스를 직접 만들지 않는다.

## 관련 모듈

- `chunk.md`
- `query.md`
- `jobs`
- `renderer`

## 메모

- 현재 최소 구현은 cube block만 렌더하고, 각 face에 `[0, 1]` UV와 texture layer index를 넣는다.
- registry에 정의되지 않은 block id는 `__missing` fallback 정의로 해석되어 흰 텍스처 위 magenta tint cube로 드러난다.
- 이 `CpuMesh`는 renderer가 직접 소유하는 타입이 아니라, 이후 jobs/app bridge에서 renderer upload DTO로 변환될 world-side CPU mesh다.
