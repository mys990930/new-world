# world

## 역할

`world`는 게임 월드의 원본 데이터와 월드 생성 계약을 소유한다.

이 모듈은 블록, 청크, 저장, 조회, 수정, 메싱 입력, 절차 생성 결과의 source of truth다. 외부
모듈은 내부 배열이나 저장 구조를 직접 만지지 않고, `world`가 노출한 API와 data contract를 통해
월드를 읽고 수정한다.

새 지형 생성 방향은 Voronoi graph 기반 macro terrain이다. 다만 상세 생성 설계는
`generation/generation.md`와 그 하위 문서가 소유한다. `world.md`는 루트 모듈의 책임 경계와
하위 문서 인덱스를 유지한다.

---

## 책임

- 로드된 청크 원본 데이터 저장
- 블록/청크 읽기, 쓰기, 조회 API
- block id, block definition, texture tile, material lookup
- save/load byte codec
- CPU-side meshing input 제공
- topdown column sampling과 진단용 preview 입력 제공
- `WorldMeta` seed, world version, generator version, save format version 계약
- deterministic procedural generation result를 `ChunkData`로 표현하는 계약
- graph-first generator가 참조하는 world-owned data contract 유지
- 기존 `world` 구현을 `legacy` 아래 보존하고, 새 generator가 대체될 때까지 runtime compatibility bridge 유지

---

## 비책임

- visible chunk 계산
- gameplay command 해석
- fixed tick scheduling
- async worker orchestration
- GPU buffer 생성과 draw/present
- OS/window/input 처리
- ECS entity state ownership

---

## 소유 데이터

### 런타임 호환 데이터

새 generator가 runtime entrypoint를 대체하기 전까지 기존 구현 데이터는 `src/world/legacy`에서
보존한다.

- `WorldMeta`
- `ChunkCoord`, `LocalBlockCoord`, `WorldBlockCoord`
- `BlockId`, `BlockFace`
- `BlockDef`, `BlockRegistry`
- `ChunkData`, `ChunkSnapshot`
- `WorldCore`
- `WorldEdit`, `EditResult`
- `CreatedWorldManifest`, `CreatedWorldSource`, `CreateWorldConfig`
- `WorldCalendar`, runtime climate/weather state
- storage, topdown, tree, surface, meshing data shapes

### 새 graph-first 생성 데이터

새 생성 데이터는 `src/world/generation` 아래에서 정의한다. 루트 `world`는 이 타입들을 필요에 따라
재노출할 수 있지만, 세부 생성 정책은 generation leaf 문서가 소유한다.

- graph region, site, corner, edge id와 patch
- continuous blended field sample
- hydrology watershed, drainage node, river segment
- generation stage, generation config, column synthesis request/result

---

## 공개 인터페이스

현재 runtime compatibility는 기존 legacy API re-export로 유지한다.

```rust
WorldCore::new(meta: WorldMeta, block_registry: Arc<BlockRegistry>) -> WorldCore
generation::generate_chunk(coord: ChunkCoord, meta: &WorldMeta, registry: &BlockRegistry) -> ChunkData
build_chunk_mesh(snapshot: &ChunkSnapshot, registry: &BlockRegistry, neighbors: NeighborChunks) -> CpuMesh
storage::load_chunk(bytes: &[u8]) -> Result<ChunkData, StorageError>
storage::save_chunk(snapshot: &ChunkSnapshot) -> Result<Vec<u8>, StorageError>
```

새 graph-first scaffold API는 `world::generation` 아래에서 확장한다.

```rust
graph_region_for_world_block(world_x: i32, world_z: i32, region_size_blocks: i32) -> GraphRegionCoord
GraphRegionArea::new(min: GraphRegionCoord, max: GraphRegionCoord) -> Option<GraphRegionArea>
VoronoiGraphPatch::site(id: VoronoiSiteId) -> Option<&VoronoiSite>

normalize_influences(influences: &mut [GraphInfluence])
ContinuousFieldSample::clamped(self) -> ContinuousFieldSample

GraphHydrologyGraph::segments_for_edge(edge: VoronoiEdgeId) -> impl Iterator<Item = &GraphRiverSegment>
graph_generation_stages() -> &'static [GraphGenerationStage]
```

---

## 의존성

허용:

- Rust standard library
- world 내부 타입
- 저장 포맷 설정
- deterministic geometry/noise helper

금지:

- `app`
- `ecs`
- `renderer`
- `platform`

`jobs`는 world 작업을 실행할 수 있지만, world 데이터 의미나 generation stage order를 소유하지
않는다.

---

## 불변식

1. block과 chunk mutation은 world-owned API를 통해서만 일어난다.
2. chunk coordinate는 저장과 출력 window일 뿐이며, macro terrain identity를 소유하지 않는다.
3. 같은 `(seed, generator_version, coord)`는 같은 generated block 결과를 내야 한다.
4. graph region, Voronoi polygon, Voronoi edge, chunk boundary는 내부 소유/계산 구조이며 visible terrain primitive가 아니다.
5. visible terrain은 hard polygon label이 아니라 blended continuous field와 warped boundary를 샘플한다.
6. river, lake, ocean, wetland는 같은 water mask로 뭉개지지 않고 의미가 구분되어야 한다.
7. 각 generation stage는 topdown preview binary로 검토 가능해야 한다.
8. legacy API는 migration bridge이며, 새 기능은 가능한 한 graph-first generation 모듈에 추가한다.

---

## 하위 문서

- `generation/generation.md`: graph-first generation 총괄, 단계 경계, preview 계약
- `generation/graph/graph.md`: Voronoi graph ownership과 graph-region coordinate 계약
- `generation/macro_map/macro_map.md`: 대륙/바다, macro elevation, 산맥/ridge/fault/coast guide
- `generation/hydrology/hydrology.md`: graph-first watershed, river, lake, local minima 계약
- `generation/boundary/boundary.md`: noisy boundary와 feature별 boundary realization
- `generation/meso_feature/meso_feature.md`: 국소 지형 feature planning과 heightfield deformation 계약
- `generation/field/field.md`: continuous blended field, moisture, biome influence 계약
- `generation/heightfield/heightfield.md`: Voronoi macro map과 Perlin micro relief 합성
- `generation/surface_plan/surface_plan.md`: biome/material/water/coast surface policy resolve
- `generation/vegetation/vegetation.md`: vegetation과 surface feature placement plan
- `generation/voxel/voxel.md`: column plan에서 `ChunkData`로 이어지는 voxel fill 계약
- `generation/preview/preview.md`: stage별 topdown preview binary 계약
- `generation/pipeline/pipeline.md`: graph-first stage order와 column synthesis scaffold
- `legacy/legacy.md`: 이전 world 구현 보존과 compatibility bridge

---

## 현재 구현 상태

- 이전 `world` 구현은 `src/world/legacy` 아래로 이동되어 보존되어 있다.
- `src/world/mod.rs`는 기존 app/tool/runtime compile을 위해 legacy API를 re-export한다.
- 새 graph-first 모듈은 `src/world/generation` 아래의 scaffold contract 단계다.
- 아직 실제 Voronoi graph 생성, Delaunay/Voronoi construction, padded graph patch assembly, macro elevation, hydrology solve, Voronoi-derived map, Perlin micro relief 합성은 구현되지 않았다.
- 새 generator entrypoint는 graph construction, field sampling, hydrology routing, heightfield synthesis, voxel fill 검증이 갖춰진 뒤 legacy generation을 대체한다.
