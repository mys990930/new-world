# world

## 역할

`world`는 게임 월드의 원본 데이터와 절차 생성 계약을 소유한다.

이 모듈의 새 방향은 기존의 사각 `atlas cell` / `chunk` 중심 지형 정체성을 버리고,
Voronoi 기반의 graph를 월드의 거시 구조로 삼는 것이다. 단, Voronoi polygon은 최종
지형 모양이 아니다. polygon, edge, corner, graph region은 소유권과 제약을 표현하는
내부 구조이며, 플레이어가 보는 높이, 바이옴, 강, 해안선, 재질 경계는 연속 필드,
spline/domain warp, noise synthesis를 거쳐 현실화되어야 한다.

핵심 원칙은 아래와 같다.

- graph는 게임플레이와 월드 일관성에 필요한 제약을 담는다.
- noise는 제약으로 고정할 필요가 없는 자연스러운 변주를 만든다.
- chunk는 저장과 출력 윈도우일 뿐, 지형 정체성의 소유자가 아니다.
- polygon 경계는 후보선이자 소유권 경계일 수 있지만, 그대로 보이는 선이어서는 안 된다.

이 방향은 Amit Patel의 Polygonal Map Generation 계열 아이디어를 이 프로젝트의
무한 복셀 월드 구조에 맞춰 재해석한 것이다.

참고:

- <https://xenon.stanford.edu/~amitp/game-programming/polygon-map-generation/>
- <https://www.redblobgames.com/maps/noisy-edges/>
- <https://www.redblobgames.com/maps/mapgen4/>
- <https://www.redblobgames.com/maps/terrain-from-noise/>

---

## 책임

- 로드된 청크 원본 데이터 저장
- 블록/청크 읽기·쓰기 API
- block id, block definition, texture tile, material lookup
- save/load byte codec
- CPU-side meshing input 제공
- top-down column sampling과 진단용 preview 입력 제공
- `WorldMeta` seed, world version, generator version, save format version 계약
- graph-first macro terrain ownership
- Voronoi site / corner / edge graph patch 생성 계약
- graph region 단위 on-demand cache/ownership 계약
- continuous field sampling 계약
- biome, surface policy, material transition의 world-side 의미 결정
- graph hydrology와 watershed ownership
- base noise heightfield와 graph-derived gradient 합성 계약
- river, lake, wetland, coast, mountain, basin 같은 macro/meso 제약의 source-of-truth
- deterministic procedural generation result를 `ChunkData`로 표현
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

## 핵심 설계 판단

### Voronoi graph는 biome graph가 아니라 world constraint graph다

Voronoi graph를 단순히 "바이옴을 나누는 polygon 모음"으로 쓰면 결국 사각형 문제가
다각형 문제로 바뀔 뿐이다. 이 프로젝트에서 graph는 더 넓은 의미를 가진다.

- site graph는 지역 소유권, 인접성, 경로 탐색, naming, gameplay region 분석에 쓴다.
- corner graph는 elevation, downhill direction, lake/sink/outlet, river routing에 쓴다.
- edge graph는 river/coast/fault/cliff/biome transition 후보선으로 쓴다.
- polygon hard owner는 query와 gameplay 안정성을 위해 보존할 수 있지만, visible terrain은 hard owner만 샘플하지 않는다.

즉, graph는 "무엇이 왜 거기에 있는가"를 정하고, noise와 field synthesis는 "그것이
어떻게 자연스럽게 보이는가"를 정한다.

### chunk는 지형 정체성을 소유하지 않는다

chunk는 최종 `ChunkData`를 저장하고 전달하기 위한 출력 단위다. 같은 world-space column은
어떤 chunk 생성 순서, 어떤 cache 재사용 경로, 어떤 y-stack 생성 방식에서도 같은 결과를
내야 한다.

따라서 generation pipeline은 chunk-local random choice를 금지한다. 필요한 randomness는
`seed + generator_version + graph owner id + feature id + world-space coordinate`에서
결정되어야 한다.

### graph region도 visible unit이 아니다

무한 월드에서는 전체 Voronoi graph를 미리 만들 수 없다. 따라서 graph region은 필요하다.
하지만 graph region은 cache/ownership 단위일 뿐이다.

- graph patch는 requested region보다 넓은 padding을 포함해야 한다.
- region 경계 근처 site, corner, edge는 생성 순서와 무관하게 안정적이어야 한다.
- graph region 사각형이 heightfield, biome, material mask에 보이면 회귀다.

---

## 소유 데이터

### 기존 runtime compatibility 데이터

이 데이터는 새 generator가 runtime entrypoint를 대체할 때까지 `src/world/legacy`에서 보존한다.

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

### graph-first 데이터

- `GraphRegionCoord`, `GraphRegionArea`
- `VoronoiSiteId`, `VoronoiCornerId`, `VoronoiEdgeId`
- `VoronoiSite`
- `VoronoiCorner`
- `VoronoiEdge`
- `VoronoiGraphPatch`
- `ContinuousFieldSample`
- `GraphInfluence`
- `VoronoiBlendSample`
- `WatershedId`
- `GraphDrainageNode`
- `GraphRiverSegment`
- `GraphHydrologyGraph`
- `GraphWorldGenerationConfig`
- `GraphGenerationStage`
- `ColumnSynthesisRequest`
- `ColumnSynthesisSample`

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

새 graph-first scaffold API는 아래 방향으로 확장한다.

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

## Graph-First Generation Model

### 1. Macro Voronoi Graph

먼저 world-space에 deterministic Voronoi-style macro graph를 만든다.

권장 방식:

- graph region 단위로 site 후보를 생성한다.
- 순수 random point는 clumping이 심하므로 Poisson Disc 또는 jittered grid + 제한된 relaxation을 우선 검토한다.
- Lloyd relaxation은 polygon 크기를 고르게 만드는 데 유용하지만, 너무 많이 적용하면 grid처럼 규칙적이 된다.
- 무한 월드에서는 region-local relaxation이 경계 불안정을 만들 수 있으므로, padding을 포함한 patch 단위 안정성 테스트가 필요하다.
- 최종 graph가 꼭 수학적으로 완전한 Voronoi일 필요는 없다. corner 간격과 polygon shape가 더 균질한 barycentric dual mesh 계열도 후보가 될 수 있다.

이 단계의 출력은 site, corner, edge, adjacency를 포함한 graph patch다.

site는 지역의 중심 의미를 가진다.

- dominant biome owner 후보
- temperature seed
- hydration seed
- elevation bias
- continentality
- ruggedness
- local naming / gameplay region owner

corner는 지형 흐름의 계산점이다.

- elevation
- downhill direction
- water accumulation
- lake/sink/outlet state
- watershed id

edge는 두 site와 두 corner 사이의 관계다.

- biome transition 후보
- coast 후보
- river 후보
- fault/cliff/plateau 후보
- road/bridge crossing 후보
- noisy boundary realization seed

### 2. Dual Graph Representation

world graph는 두 개의 연결된 graph를 가진다.

- center graph: polygon site를 node로 하고, 인접 polygon 사이를 edge로 둔다.
- corner graph: polygon corner를 node로 하고, polygon boundary segment를 edge로 둔다.

center graph는 아래에 유용하다.

- 지역 인접성
- pathfinding
- named area grouping
- biome owner propagation
- settlement/quest/strategic value analysis

corner graph는 아래에 유용하다.

- 실제 polygon boundary
- coast line
- river path
- downhill routing
- watershed
- lake outlet
- noisy edge guard geometry

하나의 Voronoi edge는 보통 `두 site + 두 corner`를 함께 알아야 한다. 이 구조를 유지해야
나중에 강은 corner edge를 따라가고, 도로는 site 사이를 가로지르는 식의 레이어 분리가 쉬워진다.

### 3. Continuous Region Fields

polygon id는 hard owner로 존재할 수 있지만, visible terrain은 hard owner를 직접 쓰지 않는다.

world-space column sampling은 주변 site/corner의 influence를 섞어 continuous field를 만든다.

- temperature
- hydration
- elevation bias
- continentality
- ruggedness
- oceanness
- mountainness
- basinness
- coastness
- fresh-water proximity

인접 site끼리는 완전 랜덤 값이 아니라 어느 정도 연속성을 가져야 한다.

방법 후보:

- graph neighbor smoothing
- low-frequency noise를 site seed에 더하기
- climate band / latitude / prevailing wind 같은 장거리 field를 site 값에 반영
- watershed, coast, mountain chain 같은 graph-derived field를 후처리로 합성

중요한 점은 `dominant_site`와 `visible field`를 분리하는 것이다. gameplay query는 안정적인
owner를 원할 수 있지만, 화면에 보이는 바이옴/재질/높이는 blended field를 먹어야 한다.

### 4. Land, Ocean, Lake, Coast

water는 단순히 `height < sea_level`로 끝내면 안 된다. world는 물의 의미를 구분해야 한다.

- ocean: 큰 바다 또는 외부 ocean basin과 연결된 물
- lake: land 내부의 local minimum 또는 basin fill로 생긴 고립 물
- wetland/marsh: 얕은 물, 높은 hydration, 낮은 slope가 겹친 지역
- coast: ocean과 land 사이의 transition band
- beach/cliff/rocky shore: coast의 slope, exposure, material policy에 따른 표면 표현

Amit의 island map에서는 border flood fill로 ocean과 lake를 구분할 수 있지만, 이 프로젝트는
무한 월드이므로 같은 방법을 그대로 쓸 수 없다. 대신 graph scale의 ocean basin ownership,
continentality field, outlet-to-ocean routing을 사용해야 한다.

Land/ocean 판정은 아래 입력을 합성한다.

- base elevation noise
- continentality
- graph basin id
- distance-to-ocean-basin
- coastness
- sea-level contract
- local lake/sink resolution

### 5. Elevation And Heightfield

전체 heightfield는 fBm/OpenSimplex/Perlin 계열의 base noise를 사용한다. 그러나 noise만으로
높이를 만들면 지형에 이유가 부족하고, hydrology가 끊기기 쉽다.

최종 높이 후보:

```text
height =
    base_low_frequency_noise
  + mid_frequency_relief_noise
  + graph_continental_gradient
  + mountain_distance_gradient
  + ridge/fault/plateau contribution
  - river_valley_carve
  - lake_basin_flatten
  + local_detail_noise
```

noise 사용 규칙:

- octave별 seed/offset/rotation을 분리해 correlation artifact를 줄인다.
- elevation redistribution curve를 둬 평지, 구릉, 산지 비율을 조절한다.
- ruggedness field가 높은 지역에서만 high-frequency amplitude를 키운다.
- mountainness는 noise threshold가 아니라 graph-derived distance field와 합성한다.
- ocean/coast/basin 영역은 noise를 그대로 두지 말고 flatten/terrace/sediment policy를 거친다.

중요한 위험:

- noise-first heightfield에는 local minima가 생긴다.
- local minima를 방치하면 강이 바다로 흐르지 않고 끊긴다.
- 따라서 hydrology 전후로 sink fill, lake creation, outlet carve 중 하나 이상의 명시적 처리가 필요하다.

### 6. Mountain And Ridge Structure

산맥은 단순히 높은 noise가 아니다.

world는 graph 위에 mountain belt / ridge chain / fault line 후보를 소유해야 한다. 구현 방식은
아래 중 하나 또는 조합이 될 수 있다.

- graph site chain을 mountain belt로 선택
- edge chain을 fault/ridge candidate로 선택
- plate-like region boundary를 uplift source로 사용
- continental core와 coast distance를 이용해 broad mountainness field 생성
- ruggedness와 elevation bias로 ridge 주변 local relief 강화

산맥은 최종 heightfield에 아래 방식으로 반영한다.

- distance-to-ridge gradient
- along-ridge variation
- pass/saddle lowering
- drainage divide ownership
- snow/alpine temperature modifier
- erosion/valley carve에 대한 저항 또는 우선순위

산맥 선이 그대로 보이면 안 된다. ridge skeleton은 broad envelope로 확산되고, domain warp와
local relief noise를 거쳐 자연스러운 능선/봉우리/안부로 바뀌어야 한다.

### 7. Hydrology Graph

Voronoi edge와 corner는 강의 후보망이다. 모든 polygon 경계가 강이 되어서는 안 된다.

hydrology 단계는 아래를 계산한다.

- corner elevation
- downhill neighbor
- sink / lake / outlet
- watershed id
- flow accumulation
- selected river segment
- river role: headwater, tributary, trunk, floodplain, outlet
- downstream progress
- approximate river width
- lake level / water surface

Amit의 mapgen2에서는 mountain corner에서 시작해 downhill 방향을 따라 ocean까지 강을 흘렸고,
여러 강이 합류하면 아래쪽 flow를 더했다. 강 폭은 flow의 제곱근 계열로 키울 수 있다. 이
아이디어는 launch generator 기본값으로 적합하다.

이 프로젝트에서는 hydrology를 아래 순서로 풀어야 한다.

1. graph corner elevation을 계산한다.
2. downhill edge를 고른다.
3. ocean outlet으로 도달하지 못하는 local minimum을 찾는다.
4. local minimum을 lake로 유지할지, outlet을 carve할지 결정한다.
5. watershed와 flow accumulation을 계산한다.
6. 충분한 flow와 지형 조건을 만족하는 edge chain만 river로 선택한다.
7. final heightfield가 river corridor를 알고 생성되도록 valley constraint를 제공한다.

최종 river geometry는 raw edge segment가 아니다.

- edge chain을 spline으로 잇는다.
- edge guard quadrilateral 안에서 noisy line을 만든다.
- river width, floodplain, gravel bar, wetland는 flow와 local slope에 따라 조절한다.
- confluence는 각진 snapping이 보이지 않도록 downstream smoothing을 적용한다.

### 8. Moisture And Hydration

hydration은 site random value 하나로 결정하지 않는다.

입력 후보:

- base climate humidity
- latitude/temperature
- prevailing wind와 rain shadow
- distance to ocean
- distance to fresh water
- lake/wetland proximity
- river flow accumulation
- elevation
- local soil/sediment class

Amit의 mapgen2는 강과 호수에서 멀어질수록 moisture가 줄어들게 했다. 이 방식은 단순하지만
바이옴 설득력이 좋다. 이 프로젝트도 launch 단계에서는 아래 모델을 우선 고려한다.

```text
hydration =
    base_humidity_field
  + fresh_water_proximity
  + wetland_bonus
  + rainfall_bonus
  - rain_shadow
  - aridity_bias
```

moisture는 원하는 분포로 redistribution할 수 있다. 예를 들어 너무 건조하거나 너무 습한
지역이 몰리면 graph patch 단위로 percentile remap을 적용할 수 있다. 단, 무한 월드에서는
patch 경계가 보이지 않도록 region padding과 deterministic window 규칙이 필요하다.

### 9. Biome Resolve

biome은 최종적으로 continuous field에서 resolve한다.

기본 축:

- temperature
- hydration
- elevation
- hydrology role
- coast/ocean/lake state
- ruggedness
- dominant graph owner

Whittaker diagram류의 temperature/moisture 2D 분류는 좋은 출발점이다. 하지만 이 프로젝트에서는
polygon 하나가 반드시 하나의 biome일 필요가 없다.

권장 방식:

- site는 dominant biome owner를 가진다.
- corner/edge/column은 blended biome influence를 가진다.
- material policy는 owner + local field + hydrology role을 함께 본다.
- 경계는 hard edge가 아니라 gradient, dithering, domain warp, cover override로 표현한다.

예외:

- gameplay상 안정적인 지역 판정이 필요한 경우 hard owner를 제공할 수 있다.
- save/load나 minimap cache가 안정적인 region id를 원할 수 있다.
- 이 경우에도 visible material boundary는 hard owner 경계를 그대로 따라가면 안 된다.

### 10. Noisy Boundary Realization

polygon boundary, coast, river, biome transition은 raw straight line으로 보이면 안 된다.

단순히 noise를 더한 spline은 교차와 찢김을 만들 수 있다. Amit의 noisy edge 아이디어에서
가장 쓸만한 부분은 boundary가 움직일 수 있는 공간을 제한하는 것이다.

하나의 Voronoi edge는 두 site와 두 corner를 가진다. 이 네 점은 boundary guard quadrilateral을
만든다.

- blue/corner edge는 polygon boundary, coast, river 후보선이 된다.
- red/site edge는 polygon center 사이의 연결선이며 road/analysis/path 등에 쓸 수 있다.
- noisy line은 guard quadrilateral 안에서 recursive subdivision 또는 spline perturbation으로 만든다.
- 같은 edge id와 seed는 언제나 같은 noisy line을 만든다.
- neighboring chunk가 같은 edge를 샘플하면 같은 line을 얻어야 한다.

이 구조는 아래에 필요하다.

- biome boundary
- coast line
- river centerline
- cliff/fault line
- road/bridge relation

boundary 표현은 feature마다 다를 수 있다.

- biome boundary: gradient/dither/domain warp 중심
- river: spline corridor + width/floodplain
- coast: noisy coastline + beach/cliff material policy
- fault/cliff: 부분적으로 discontinuous height transition 허용

### 11. Roads, Bridges, And Cross-Graph Features

원문에서 흥미로운 분리는 "강은 Voronoi edge를 따라가고, 도로는 Delaunay edge를 따라간다"는
것이다. 이 프로젝트에서도 나중에 쓸 가치가 있다.

- 강은 corner graph edge를 따라 흐른다.
- 도로, 경로, 동물 이동로는 site graph edge 또는 contour-crossing path로 만든다.
- 도로가 river edge를 crossing하면 bridge 후보가 명확해진다.
- pathfinding상 가까운데 실제 경로가 먼 지점은 bridge/tunnel/ford 후보가 된다.

이 레이어는 launch generator의 필수 범위는 아니지만, graph schema가 막아서는 안 된다.

### 12. Watersheds And Named Areas

downhill edge를 따라가면 각 corner는 어떤 outlet 또는 lake/sink에 도달한다. 같은 outlet을
공유하는 corner와 polygon 묶음은 watershed가 된다.

watershed는 단순 hydrology 결과 이상의 가치가 있다.

- 강 이름
- 산맥 이름
- 계곡 이름
- 호수 이름
- 숲/습지/해안 지역 이름
- quest/loot/settlement 배치의 지역 맥락

예를 들어 같은 watershed 안에서 `XYZ River`, `XYZ Valley`, `Mount XYZ` 같은 연관 이름을
만들 수 있다. 이 프로젝트가 생활/생태계/탐험 샌드박스를 목표로 한다면, watershed 기반
named area는 장기적으로 중요한 시스템이 될 수 있다.

### 13. Impassable Or Discontinuous Borders

모든 polygon 경계가 부드럽게 이어질 필요는 없다. 일부 edge는 gameplay와 지형 정체성을 위해
불연속성을 가질 수 있다.

후보:

- cliff
- chasm
- plateau step
- fault scarp
- canyon wall
- lava fissure
- glacier crevasse

이 edge들은 visible boundary가 될 수 있지만, 그래도 raw polygon edge가 그대로 보이면 안 된다.
noisy boundary, local erosion, talus/sediment, vegetation mask를 통해 자연스럽게 현실화해야 한다.

### 14. Variable Density

모든 지역에 같은 graph density를 쓸 필요는 없다.

후보 정책:

- 보통 야생 지역은 coarse graph
- coast, river confluence, settlement 후보, dungeon 주변은 finer graph
- 큰 ocean이나 평야는 낮은 density
- 산맥, 계곡, 습지, 경계 지형은 높은 density

다만 variable density는 무한 월드에서 어려운 문제다. site spacing이 region 경계에서 바뀌면
seam이 생길 수 있다. 따라서 launch 단계에서는 고정 density를 먼저 쓰고, graph schema만
variable density를 막지 않게 열어둔다.

### 15. Terrain Analysis

polygon graph는 빠른 terrain analysis에 유용하다.

분석 후보:

- shortest path와 euclidean distance 차이
- chokepoint
- bridge/tunnel 후보
- coast 접근성
- mountain pass
- river crossing
- frequently-used path region
- isolated valley
- strategic settlement candidate

이 분석은 `ecs`가 아니라 `world` 또는 향후 world-owned analysis layer가 소유해야 한다. ECS는
그 결과를 gameplay 의미로 해석할 수 있지만, 원본 지형 graph와 접근성 계산은 world 데이터에
가깝다.

### 16. Module Annotation Model

graph core에 모든 feature field를 직접 박아 넣으면 빠르게 비대해진다. Amit의 원문에서
가져올 만한 구조는 "core graph는 index/id를 제공하고, feature module은 외부 table로 annotate한다"는 방식이다.

권장 방향:

- `VoronoiGraphPatch`는 site/corner/edge id와 topology를 소유한다.
- hydrology는 `edge_id -> river segment`, `corner_id -> drainage node` 같은 별도 layer를 가진다.
- biome은 `site_id -> biome owner`, `column -> blended influence` 별도 layer를 가진다.
- boundary realization은 `edge_id -> noisy curve` cache를 가진다.
- roads/faults/lava/ecology 같은 feature는 core graph에 직접 의존하지 않고 id 기반 layer로 붙는다.

이 방식은 world 내부에서도 의존성을 줄인다. `graph`는 `hydrology`를 몰라도 되고, `hydrology`는
필요한 graph id와 샘플만 읽는다.

---

## 권장 처리 흐름

새 generator의 목표 pipeline은 아래 순서다.

1. target chunk 또는 preview area가 필요한 world-space x/z 범위를 정한다.
2. 이 범위를 덮는 padded graph region area를 계산한다.
3. deterministic site 후보를 생성한다.
4. site 분포를 안정화한다. launch에서는 Poisson Disc 또는 jittered grid를 우선한다.
5. Voronoi/Delaunay dual graph patch를 만든다.
6. site/corner/edge에 macro seed field를 부여한다.
7. neighbor smoothing과 low-frequency field로 continuous climate/terrain seed를 만든다.
8. land/ocean/lake/coast 후보를 계산한다.
9. corner elevation과 downhill direction을 계산한다.
10. local minima를 lake/sink/outlet carve로 처리한다.
11. watershed와 flow accumulation을 계산한다.
12. selected river edge chain을 만든다.
13. mountain/ridge/fault/coast gradient를 만든다.
14. column별 blended field sample을 만든다.
15. base noise heightfield와 graph-derived gradient를 합성한다.
16. river valley, lake flattening, floodplain, wetland를 반영한다.
17. biome/material/surface policy를 blended field와 hydrology role에서 resolve한다.
18. `ChunkData`로 voxel fill한다.

---

## TDD / 검증 기준

새 generator는 구현보다 먼저 검증 표면을 가져야 한다.

### Determinism

- 같은 `(seed, generator_version, graph_region)`은 같은 graph patch를 만든다.
- 같은 `(seed, generator_version, world_x, world_z)`는 같은 column sample을 만든다.
- chunk별 생성과 area/stack batch 생성 결과가 일치한다.

### Seam

- 인접 chunk 경계 column이 일치한다.
- 인접 graph region 경계에서 site/corner/edge ownership이 안정적이다.
- padded graph patch를 다르게 요청해도 overlap 영역의 graph-derived sample이 같다.

### Artifact Suppression

- graph region 사각 경계가 height/material/biome에 보이지 않는다.
- raw Voronoi edge가 그대로 직선 river/coast/biome boundary로 보이지 않는다.
- noisy boundary가 edge guard 영역 밖으로 나가거나 이웃 edge와 교차하지 않는다.
- hard owner와 visible material boundary가 과도하게 같은 선을 따라가지 않는다.

### Hydrology

- river segment는 downstream progress를 가진다.
- flow accumulation은 합류 후 증가한다.
- selected river는 lake/sink/outlet 처리 없이 끊기지 않는다.
- local minima는 lake, sink, outlet carve 중 하나로 명시된다.
- river width는 flow와 안정적으로 연결된다.

### Field Continuity

- 인접 site의 temperature/hydration/elevation bias는 비현실적으로 튀지 않는다.
- biome transition은 gradient 또는 domain warp를 통해 완만하게 변한다.
- ocean/coast/lake/wetland 구분은 material policy와 topdown preview에서 일관된다.

### Visualization

초기 구현은 반드시 진단 출력을 포함해야 한다.

- site/corner/edge graph preview
- graph region ownership preview
- dominant site map
- blended influence map
- elevation/corner downhill arrow map
- watershed map
- river flow accumulation map
- noisy edge preview
- final heightfield preview
- biome/material preview

이 프로젝트는 문서와 테스트가 source of truth이므로, 시각화에서 이상한 작은 흔적이 보이면
무시하지 않고 추적해야 한다.

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

`jobs`는 graph/generation 작업을 실행할 수 있지만, graph/generation 의미를 소유하지 않는다.

---

## 불변식

1. block과 chunk mutation은 world-owned API를 통해서만 일어난다.
2. chunk coordinate는 출력 window일 뿐이며, macro terrain identity를 소유하지 않는다.
3. graph region, Voronoi polygon, Voronoi edge, chunk boundary는 내부 소유/계산 구조이며 visible terrain primitive가 아니다.
4. visible terrain은 hard polygon label이 아니라 blended continuous field와 warped boundary를 샘플한다.
5. Voronoi edge는 hydrology 후보선이지 자동 river가 아니다.
6. river, lake, ocean, wetland는 같은 water mask로 뭉개지지 않고 의미가 구분되어야 한다.
7. hydrology는 최종 heightfield와 voxel fill 전에 제약으로 반영되어야 한다.
8. noise-first heightfield를 사용할 경우 local minima 처리가 명시되어야 한다.
9. selected river path는 generation order와 chunk order에 독립적이어야 한다.
10. graph-derived mountain/ridge/coast guide는 broad field로 확산되어야 하며 raw segment가 그대로 보이면 안 된다.
11. biome owner와 visible material boundary는 분리될 수 있어야 한다.
12. 같은 `(seed, generator_version, coord)`는 같은 generated block 결과를 내야 한다.
13. legacy API는 migration bridge이며, 새 기능은 가능한 한 graph-first 모듈에 추가한다.

---

## 하위 문서

- `graph.md`: Voronoi graph ownership과 graph-region coordinate 계약
- `field.md`: continuous blended field sampling 계약
- `hydrology.md`: graph-first watershed와 river-edge 계약
- `pipeline.md`: graph-first generation stage order와 column synthesis scaffold
- `legacy/legacy.md`: 이전 world 구현 보존과 compatibility bridge

---

## 현재 구현 상태

- 이전 `world` 구현은 `src/world/legacy` 아래로 이동되어 보존되어 있다.
- `src/world/mod.rs`는 기존 app/tool/runtime compile을 위해 legacy API를 re-export한다.
- 새 graph-first 모듈은 현재 scaffold contract 단계다.
- 아직 실제 Voronoi graph 생성, Delaunay/Voronoi construction, padded graph patch assembly, hydrology solve, noise heightfield 합성은 구현되지 않았다.
- 새 generator entrypoint는 graph construction, field sampling, hydrology routing, heightfield synthesis, voxel fill 검증이 갖춰진 뒤 legacy generation을 대체한다.

---

## 우선 구현 순서

1. graph region과 deterministic site generation
2. padded graph patch와 overlap determinism 테스트
3. site/corner/edge topology 생성
4. graph preview dump
5. continuous field sampling
6. land/ocean/lake/coast classification prototype
7. corner elevation과 downhill routing
8. local minima 처리
9. watershed/flow accumulation
10. selected river edge chain과 noisy river curve
11. base noise heightfield + graph gradient 합성
12. chunk voxel fill
13. topdown/debug preview와 seam regression 테스트
