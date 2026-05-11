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
- cell biome, chunk-scoped weather scalar state, surface condition, and world update observation data for diagnostics and later renderer/gameplay consumers
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
- text formatting or console output

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
- chunk weather scalar state: temperature, moisture, cloud, rain, derived weather kind, and update tick
- cell biome / region classification cache used by HUD, minimap, and textmode observers
- chunk or cell surface condition state such as wet, snow-covered, and half-thawed snow
- structured world update records when simulation/apply paths request or apply changes
- storage, topdown, tree, surface, meshing data shapes

### 새 graph-first 생성 데이터

새 생성 데이터는 `src/world/generation` 아래에서 정의한다. 루트 `world`는 이 타입들을 필요에 따라
재노출할 수 있지만, 세부 생성 정책은 generation leaf 문서가 소유한다.

- graph region, site, corner, edge id와 patch
- graph base `continentality/elevation_seed`와 macro ownership/elevation resolve annotation
- graph region cache, macro map cache, hydrology/river-plan/final-cell-context/boundary/macro field/heightfield cache key와 cached stage output
- continuous blended field sample
- hydrology watershed, drainage node, river segment
- river plan chain/reach morphology, broad valley parameter, river bed hint
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
WorldCore::sample_cached_region_class_atlas(coord: AtlasCoord) -> Option<RegionClassSample>
WorldCore::local_weather(coord: AtlasCoord) -> Option<LocalWeatherState>
WorldCore::chunk_weather(coord: ChunkCoord) -> Option<ChunkWeatherState>
WorldCore::apply_chunk_weather_update(update: ChunkWeatherUpdate) -> WeatherApplyResult
WorldCore::chunk_surface_condition(coord: ChunkCoord) -> SurfaceCondition
WorldCore::set_chunk_surface_condition(coord: ChunkCoord, condition: SurfaceCondition) -> Option<SurfaceCondition>
WorldCore::observe_chunk_surface_condition(coord: ChunkCoord) -> SurfaceConditionObservation
WorldCore::apply_calendar_advance(advance: CalendarAdvance) -> CalendarApplyResult
WorldCore::apply_edit(edit: WorldEdit) -> EditResult
```

새 graph-first scaffold API는 `world::generation` 아래에서 확장한다.

```rust
graph_region_for_world_block(world_x: i32, world_z: i32, region_size_blocks: i32) -> GraphRegionCoord
GraphRegionArea::new(min: GraphRegionCoord, max: GraphRegionCoord) -> Option<GraphRegionArea>
VoronoiGraphConfig::new(seed: u64, generator_version: u32) -> VoronoiGraphConfig
VoronoiGraphPatchRequest::new(config, center_world_x, center_world_z) -> VoronoiGraphPatchRequest
generate_voronoi_graph_patch(request: VoronoiGraphPatchRequest) -> VoronoiGraphPatch
apply_base_graph_fields(patch: &mut VoronoiGraphPatch, config: GraphBaseFieldConfig)
VoronoiGraphPatch::site(id: VoronoiSiteId) -> Option<&VoronoiSite>
MacroMapConfig::new(seed: u64, generator_version: u32) -> MacroMapConfig
generate_macro_map(patch: &VoronoiGraphPatch, config: MacroMapConfig) -> GraphMacroMap
GraphMacroMap::coast_edges() -> impl Iterator<Item = &MacroEdge>
GraphMacroMap::river_candidate_edges() -> impl Iterator<Item = &MacroEdge>

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
6. river, lake, ocean, wetland, dry basin은 같은 water mask로 뭉개지지 않고 의미가 구분되어야 한다.
   patch/guard boundary 접촉만으로 ocean을 만들지 않고, explicit ocean basin 연결성을 기준으로
   ocean과 inland lake/wetland를 구분해야 한다.
7. 각 generation stage는 topdown preview binary로 검토 가능해야 한다.
8. legacy API는 migration bridge이며, 새 기능은 가능한 한 graph-first generation 모듈에 추가한다.
9. 실시간 chunk fill은 graph/macro/hydrology stage를 chunk마다 재계산하지 않고, world-owned
   generation cache를 읽어야 한다.
10. textmode observer data must be world-readable structured state, not console-only strings.
11. cell biome labels used by diagnostics must derive from world-owned region/biome classification, not from app-side ad-hoc names.
12. chunk weather scalar state is world-owned storage; simulation computes updates and renderer/textmode consume the same values.

---

## 하위 문서

- `generation/generation.md`: graph-first generation 총괄, 단계 경계, preview 계약
- `generation/graph/graph.md`: Voronoi graph ownership과 graph-region coordinate 계약
- `generation/macro_map/macro_map.md`: 대륙/바다, macro elevation, ridge/fault/coast guide와 mountainness/rugged context
- `generation/hydrology/hydrology.md`: graph-first watershed, river, lake, local minima 계약
- `generation/river_plan/river_plan.md`: selected river를 reach morphology와 broad valley / narrow bed plan으로 번역하는 계약
- `generation/boundary/boundary.md`: 모든 Voronoi edge의 canonical noisy geometry
- `generation/meso_feature/meso_feature.md`: 국소 지형 feature planning과 heightfield deformation 계약
- `generation/field/field.md`: continuous blended field, moisture, biome influence 계약
- `generation/biome/biome.md`: graph-first final cell biome context와 classification 계약
- `generation/macro_field/macro_field.md`: graph-derived signed distance / influence field tile cache
- `generation/heightfield/heightfield.md`: macro field와 Perlin micro relief 합성
- `generation/surface_plan/surface_plan.md`: biome/material/water/coast surface policy resolve
- `generation/vegetation/vegetation.md`: vegetation과 surface feature placement plan
- `generation/voxel/voxel.md`: column plan에서 `ChunkData`로 이어지는 voxel fill 계약
- `generation/preview/preview.md`: stage별 topdown preview binary 계약
- `generation/pipeline/pipeline.md`: graph-first stage order와 column synthesis scaffold
- `legacy/legacy.md`: 이전 world 구현 보존과 compatibility bridge
- `legacy/surface/condition.md`: textmode/renderer/gameplay consumers가 읽는 surface condition 관찰 계약
- `../simulation/weather.md`: chunk weather scalar state, biome ranges, seasonal coefficients, thresholds, and renderer contract

---

## 현재 구현 상태

- 이전 `world` 구현은 `src/world/legacy` 아래로 이동되어 보존되어 있다.
- `src/world/mod.rs`는 기존 app/tool/runtime compile을 위해 legacy API를 re-export한다.
- 새 graph-first 모듈은 `src/world/generation` 아래의 scaffold contract 단계다.
- 현재 `graph` leaf는 deterministic padded site 후보를 `delaunator` Delaunay triangulation으로
  연결하고, triangle circumcenter를 Voronoi corner로 삼는 Voronoi dual graph patch assembly와
  base graph field smoothing을 제공한다.
- 현재 `macro_map` leaf는 graph patch 기반 continent/ocean basin ownership, signed macro elevation,
  island/archipelago component, coast/ridge/fault guide annotation을 제공한다. 목표 계약상 이 ownership과
  elevation은 graph base `continentality/elevation_seed`를 source of truth로 resolve하며,
  macro_map은 독자 continent/island noise source를 만들지 않는다.
- 현재 `hydrology` leaf는 macro elevation/coast guide/graph topology 기반 downhill, watershed,
  flow accumulation, selected river segment scaffold를 제공한다.
- `river_plan`은 문서 전용 단계로 추가되었으며, hydrology selected river를 reach type, broad valley,
  narrow bed hint로 번역하는 책임을 소유한다. 현재 구현은 아직 `macro_field` 내부 river influence가
  일부 morphology 계산을 직접 수행한다.
- 현재 `heightfield` leaf는 `MacroFieldTile`을 column-oriented heightfield cache로 변환하는 vertical
  slice를 제공한다. meso feature와 Perlin micro relief는 아직 `0` stub이다.
- 아직 구현되지 않은 것: Perlin micro relief 실제 합성, final surface/material resolve, voxel fill 연결.
- 새 generator entrypoint는 graph construction, field sampling, hydrology routing, heightfield synthesis, voxel fill 검증이 갖춰진 뒤 legacy generation을 대체한다.
- planned `new-world-textmode` support should expose a structured per-chunk observer view containing cell biome, chunk weather scalar state, surface condition, ecology events, and world update records while keeping console formatting outside `world`.
- old atlas `LocalWeatherState` remains a migration bridge until chunk-scoped weather state replaces weather consumers.
