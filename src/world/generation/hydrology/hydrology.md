# hydrology

## 역할

`hydrology`는 Voronoi corner와 selected edge를 기반으로 한 graph-first 물 흐름 계약을 소유한다.

polygon 경계는 river path가 될 수 있는 graph substrate지만, 모든 경계가 강이 되어서는 안 된다.
hydrology는 macro_map이 graph base field에서 resolve한 ownership/elevation, ridge, coast, basin
정보를 읽어 downhill routing, watershed, selected river segment, lake/outlet 처리를 계산하고,
이후 `river_plan`이 reach morphology를 정하며 heightfield가 valley와 water surface를 알 수 있게
제약을 제공한다.

---

## 책임

- watershed, drainage node, river segment 표현
- selected graph edge를 headwater, tributary, trunk, floodplain, outlet으로 분류
- flow accumulation과 downstream progress 유지
- lake, sink, outlet carve 같은 local minima 처리 계약 정의
- river_plan, heightfield와 surface plan이 읽을 selected river/lake/water constraint 제공
- macro_map의 `MacroLakeEdgeClass`를 읽어 selected river가 lake internal/boundary/adjacent edge를
  쓰지 않도록 강제

---

## 비책임

- base noise heightfield 생성
- final voxel channel carve
- sediment 또는 surface material 선택
- noisy river spline curve 생성
- reach별 valley width/depth, river bed width/depth, bank/floodplain morphology 결정
- live world storage mutation

---

## Hydrology Graph

hydrology 단계는 아래를 계산한다.

- corner elevation
- downhill neighbor
- sink / lake / outlet
- watershed id
- raw flow accumulation
- selected/display discharge
- selected river segment
- lake inlet/outlet vertex
- topology validation stats
- downstream progress
- terminal role and selected/display discharge hints for `river_plan`
- lake level / water surface

Amit의 mapgen2에서는 mountain corner에서 시작해 downhill 방향을 따라 ocean까지 강을 흘렸고,
여러 강이 합류하면 아래쪽 flow를 더했다. 강 폭은 flow의 제곱근 계열로 키울 수 있다. 이
아이디어는 launch generator 기본값으로 적합하다.

---

## 공개 API

현재 구현은 graph patch와 macro map을 입력으로 받아 별도 hydrology annotation layer를 만든다.
graph core와 macro_map에는 selected river state를 직접 쓰지 않고, corner/edge id 기반 table로
결과를 반환한다.

```rust
HydrologyConfig::default() -> HydrologyConfig
solve_hydrology(&VoronoiGraphPatch, &GraphMacroMap, HydrologyConfig) -> GraphHydrologyGraph

GraphHydrologyGraph {
    corners: Vec<GraphHydrologyCorner>,
    nodes: Vec<GraphDrainageNode>,
    segments: Vec<GraphRiverSegment>,
    topology_stats: GraphHydrologyTopologyStats,
}

GraphHydrologyCorner {
    id,
    elevation,
    downstream,
    downstream_edge,
    watershed,
    flow_accumulation,
    is_local_minimum,
    resolution,
}

GraphHydrologyTopologyStats {
    lake_inlet_count,
    lake_outlet_count,
    disconnected_lake_inlet_count,
    disconnected_lake_outlet_count,
    selected_lake_edge_segment_count,
    invalid_lake_contact_count,
    invalid_river_intersection_count,
    ambiguous_shared_corner_count,
    duplicate_trunk_pruned_count,
    repeated_lake_contact_pruned_count,
}
```

`GraphRiverSegment`는 potential guide가 아니라 selected river result다. preview와 이후
heightfield는 이 segment만 강으로 해석해야 한다. macro_map의 ridge/fault/coast guide는 이 단계의
입력일 뿐이며, pre-hydrology river candidate와 혼동하면 안 된다.
corner의 `flow_accumulation`은 hydrology 원장에 가까운 raw accumulation이며, river segment는
`raw_flow_accumulation`과 정책 적용 후의 `flow_accumulation`을 함께 가진다. preview의 강 두께와
초기 river width는 segment의 정책 적용 후 `flow_accumulation`을 사용한다.

---

## 처리 순서

1. macro_map의 resolved ownership/elevation, ridge guide, coast guide를 읽는다.
2. graph corner elevation을 계산한다.
3. downhill edge를 고른다.
4. graph-stage local minimum을 찾는다.
5. local minimum을 lake로 유지할지, sink로 둘지, outlet을 carve할지 결정한다.
6. watershed와 flow accumulation을 계산한다.
7. terminal 정책을 적용해 충분한 flow와 지형 조건을 만족하는 edge chain만 selected river로 선택한다.
8. lake contact topology를 정리한다.
   - selected river는 `MacroLakeEdgeClass::{LakeInternal,LakeBoundary,LakeAdjacentLand}` edge를
     어떤 경우에도 사용하지 않는다. 이 판정은 corner surface만 보지 않고 macro edge의 인접 site와
     endpoint corner annotation을 함께 읽은 결과다.
   - 유입하천은 lake boundary edge 직전의 land-side selected endpoint에서 `LakeInlet` node로
     종료된다. 해당 node는 반드시 incoming selected segment를 가져야 하며, lake edge segment
     자체는 selected river가 아니다.
   - 유출하천은 같은 lake component의 다른 boundary vertex 밖에 있는 land-side selected endpoint에서
     `LakeOutlet` node로 시작된다. 해당 node는 반드시 outgoing selected segment를 가져야 한다.
   - outlet vertex는 selected inlet이 lake로 들어오기 직전의 land-side approach corner elevation보다 낮아야 한다.
   - outlet vertex는 inlet vertex와 같은 corner가 아니며, 기본값 기준 최소 2 lake-edge hop 이상 떨어져야 한다.
9. selected river graph의 shared corner를 검증한다.
   - 둘 이상의 selected segment가 한 corner에서 만나는 경우는 downstream confluence 또는 명시 terminal로 설명 가능해야 한다.
   - branch를 명시적으로 모델링하기 전까지 selected graph는 한 corner에서 여러 독립 chain이 교차하는 형태를 제거한다.
   - launch preview 정책은 ambiguous shared corner를 보수적으로 다룬다. 같은 corner로 여러 selected
     incoming chain이 들어오면 raw flow가 가장 큰 winner만 selected로 남기고, 나머지 upstream selected
     tree는 제거한다. 이렇게 해서 flow accumulation 원장은 합류를 보존하되, selected river overlay는
     별도 강줄기가 같은 꼭짓점을 공유하며 겹쳐 보이지 않게 한다.
10. selected river chain이 ocean outlet, 명시적인 lake/sink, 또는 downstream portal/outlet carve 없이 끊기지 않도록 검증한다.
11. `river_plan`이 reach morphology를 만들 수 있도록 selected river segment, flow, downstream
    progress, lake/sink/outlet terminal role을 제공한다.

launch 구현은 아래의 보수적인 정책을 사용한다.

- terminal outlet은 connected ocean/coast corner 또는 coast guide와 인접한 corner다. signed macro
  elevation이 음수라는 이유만으로 ocean outlet이 되지는 않는다.
- 일반 corner는 인접 corner 중 더 낮은 elevation 또는 ocean/coast terminal을 downhill target으로 고른다.
- 더 낮은 이웃이 없는 graph-stage local minimum은 spill path search를 수행한다.
- local minimum이 `LakeCandidate` 또는 `WetlandCandidate` 위에 있으면 ocean으로 carve하기 전에
  explicit lake resolution을 우선 적용한다.
- 모든 graph-stage local minimum을 lake로 만들지는 않는다. macro_map이 `DryBasin`으로 분류한
  폐쇄 저지대나 spill path가 없는 분지는 explicit `Sink`로 남을 수 있고, ocean/coast까지 낮은
  spill path가 있으면 `OutletCarve`가 우선될 수 있다.
- spill path가 ocean/coast terminal까지 닿으면 outlet carve로 downstream edge chain을 만든다.
- spill path가 없으면 explicit sink로 남긴다. selected river는 explicit sink를 제외하고 중간에서 끊기면 안 된다.
- flow accumulation은 land corner rainfall contribution을 downstream으로 누적한다.
- selected river는 threshold를 넘은 headwater에서 시작하되, 선택된 순간 downstream chain을 outlet/sink/lake까지 계속 포함한다.
- ocean outlet으로 이어지는 river는 raw/selected display flow accumulation을 기준으로 넓어질 수 있다.
  이후 macro_field/heightfield 단계에서 이 값은 width와 depth를 함께 키운다. 상류는 좁고 얕고,
  하류 trunk는 넓고 깊어야 하며, launch preview에서 모든 selected river가 같은 폭으로 보이면 회귀다.
- lake로 끝나는 river와 lake/wetland component로 처음 들어가는 inlet river는 raw flow accumulation을
  보존하되 lake 면적에서 파생한 capacity를 기준으로 selected incoming chain 수, visible inlet segment
  수, 표시/폭 계산용 discharge를 제한한다. 기본 정책은 lake/wetland candidate corner 수를 `area_units`로 보고,
  `max_lake_terminal_chains = min(3, 1 + floor(area_units / 24))`를 적용한다. 작은 lake는 1개
  이하의 feeder chain만 보이고, 큰 lake도 ocean outlet river network처럼 많은 지류를 먹지 않는다.
- lake terminal/inlet chain은 lake area와 display cap에서 파생한 raw flow threshold를 넘어야 선택된다.
  작은 lake는 작은 feeder를 허용하되 너무 자잘한 흐름은 marker로 승격하지 않고, 큰 lake는 더 큰
  raw feeder를 요구한다. 선택된 lake-bound chain은 더 이상 호수 직전 몇 segment로 잘리지 않는다.
  lake edge 자체는 계속 금지하지만, 기준을 통과한 기존 upstream trunk는 lake boundary 직전
  land-side endpoint까지 selected river로 유지될 수 있다.
- 1~3 site/cell tiny local-minima lake는 macro_map이 land-owned graph 저지대에서 낮은 확률로 만든
  작은 lake 후보를 읽는다. 이 후보는 river-side 조건 없이 생길 수 있으므로 hydrology는 selected
  flow가 없더라도 lake footprint 자체를 유지한다. selected flow가 연결되면 lake boundary edge를 쓰지
  않는 land-side endpoint에서 `LakeInlet` 또는 `LakeOutlet`으로 분류되어야 하며, 연결된 selected
  flow가 marker 없이 남으면 `unclassified_lake_connected_flow_count` 회귀로 잡힌다.
- lake terminal/inlet display discharge는 lake 면적에 따라 범위가 함께 올라간다. launch 기본 cap은
  `min(32, 4 + area_units * 0.35)`이고, display floor는 `min(32 * 0.55, 4 * 0.55 + area_units * 0.18)`이다.
  raw accumulation은 `raw_flow_accumulation`에 보존하지만, preview width/opacity와 초기 river width는
  이 lake-area display band를 통과한 `flow_accumulation`을 사용한다. 따라서 lake terminal river는
  일반 ocean outlet trunk보다 확연히 얇고 적되, lake 크기가 커질수록 inlet/outlet discharge range도
  같이 커진다.
- lake 유입/유출 topology는 visual artifact 방지를 위해 selected graph 단계에서 고정된다. lake로 들어가는
  흐름은 `MacroLakeEdgeClass`가 lake 관련 edge로 분류한 edge를 selected segment로 쓰지 않는다.
  `LakeInlet`은 lake boundary 바로 바깥의
  land-side selected endpoint에 붙고 incoming selected segment가 있으며 raw flow가 lake 면적/capacity
  기반 inlet threshold 이상일 때만 생성된다. 작은 feeder는 raw ledger와 selected
  segment에는 남을 수 있지만 preview-visible inlet marker로 승격되지 않는다. `LakeOutlet`은 lake별
  0개부터 최대 2개까지 허용하는 launch 계약을 가진다. 현재 구현은 가장 낮고 유입부에서 떨어진 후보를
  우선해 lake별 0개 또는 1개 outlet을 고른다. `LakeOutlet`도
  lake boundary 바로 바깥의 land-side selected endpoint에 붙고 outgoing selected segment가 있을 때만
  생성된다. 실제 lake boundary corner는 marker 방향과 lake component pairing에만 쓰이며, selected
  river segment endpoint가 되지 않는다. outlet corner의 높이 비교는 lake surface vertex가 아니라
  유입하천의 land-side approach corner elevation을 기준으로 한다. 이는 lake 후보 corner들이 같은
  수면/분지 값으로 평탄해질 수 있기 때문이다.
- 같은 selected river chain은 lake와 두 번 접촉하지 않는다. lake inlet에서 끝난 chain과 lake outlet에서
  시작하는 chain은 별도 chain으로 취급한다. outlet에서 시작한 chain이 다른 lake contact에 다시 닿으면
  launch 정책은 그 selected outlet chain을 제거하고 `repeated_lake_contact_pruned_count`에 기록한다.
- lake boundary 바깥의 selected flow endpoint가 lake와 연결되어 있는데 `LakeInlet` 또는 `LakeOutlet`
  marker로 분류되지 않으면 회귀다. launch 구현은 이런 endpoint를
  `unclassified_lake_connected_flow_count`로 계측하고 정상 solve에서 0을 요구한다.
- selected river occupancy는 `one selected outgoing per corner`인 downhill graph 위에서, preview-visible
  incoming도 기본적으로 `one incoming per corner`가 되도록 정리한다. 자연스러운 대규모 합류를 별도
  confluence geometry로 표현하기 전까지는 여러 headwater가 같은 trunk vertex에 따로 붙는 형태보다
  가장 큰 selected branch 하나를 남기는 쪽을 우선한다.

최종 river morphology는 hydrology가 직접 만들지 않는다.

- hydrology edge chain은 river topology다.
- `river_plan`은 selected chain의 downstream progress와 selected/display discharge를 읽어
  `headwater`, `upper`, `middle`, `lower`, `trunk`, `lake inlet/outlet` 같은 reach type을 정한다.
- river width, broad valley, bed depth, bank/floodplain parameter는 `river_plan`이 flow와 local
  role에 따라 결정한다.
- lake terminal/inlet river의 morphology는 raw accumulation이 아니라 lake capacity가 적용된 selected
  discharge와 lake terminal role을 우선 사용한다. raw flow는 hydrology ledger와 inlet threshold 판정에
  남고, display flow는 lake area에 비례한 cap을 통과한 값이다.
- confluence smoothing이나 visible river axis 조정은 hydrology가 아니라 `river_plan` 또는 downstream
  field/heightfield stage의 책임이다.

---

## Local Minimum

hydrology 문서에서 말하는 local minimum은 기본적으로 Perlin 이후 pixel/column depression이 아니라
graph-stage minimum이다.

- corner local minimum: 어떤 corner가 인접 corner 중 더 낮은 downstream 후보를 찾지 못하는 경우
- basin local minimum: 여러 corner/site가 같은 닫힌 저지대로 모여 ocean outlet을 갖지 못하는 경우
- cell local minimum: site 중심이 주변보다 낮아 분지처럼 보이지만, 실제 routing은 corner graph로 검증해야 하는 경우

graph-stage local minimum은 `lake`, `sink`, `outlet carve` 중 하나로 명시한다. launch 기본값은
큰 selected river가 아무 설명 없이 끊기는 것을 금지한다. selected river는 ocean outlet에 닿거나,
명시적인 lake/sink에 도달하거나, outlet carve로 다음 downstream portal을 가져야 한다.

Perlin micro relief 이후 생기는 작은 column depression은 다른 문제다. 이것은 macro hydrology를
다시 라우팅하는 원인이 되어서는 안 된다. heightfield 단계에서 clamp, local fill, wetland/puddle
표현, river/lake 주변 flatten 중 하나로 처리한다.

---

## Procedural Downstream Continuity

무한 월드에서는 요청된 chunk가 downstream 전체를 미리 알 수 없다. 따라서 river chain 안정성은
전체 월드를 즉시 생성해서 보장하는 방식이 아니라 deterministic ownership과 lazy downstream
contract로 보장한다.

필요한 정책:

- hydrology solve는 요청 영역보다 넓은 padded graph patch에서 실행한다.
- 각 graph region은 border portal과 downstream basin id를 가질 수 있다.
- selected river가 patch 밖으로 나가면 open downstream portal을 기록하고, 다음 region이 생성될 때 같은 seed와 basin id로 이어받는다.
- ocean basin ownership과 drainage potential field는 local sample만으로도 대략적인 downstream 방향을 제공해야 한다.
- launch에서 "모든 주요 river는 바다로 연결"을 원하면, 닫힌 basin은 lake로 남기지 않고 outlet carve를 선택해 ocean basin까지 이어지는 portal chain을 만든다.

즉, 다음 cell을 아직 메모리에 갖고 있지 않아도, seed 기반 graph region과 downstream portal 계약이
동일하면 나중에 생성되는 하류는 같은 위치에서 이어진다.

---

## Watersheds And Named Areas

downhill edge를 따라가면 각 corner는 어떤 outlet 또는 lake/sink에 도달한다. 같은 outlet을
공유하는 corner와 polygon 묶음은 watershed가 된다.

watershed는 단순 hydrology 결과 이상의 가치가 있다.

- 강 이름
- 산맥 이름
- 계곡 이름
- 호수 이름
- 숲/습지/해안 지역 이름
- quest/loot/settlement 배치의 지역 맥락

예를 들어 같은 watershed 안에서 `XYZ River`, `XYZ Valley`, `Mount XYZ` 같은 연관 이름을 만들 수
있다. 이 프로젝트가 생활/생태계/탐험 샌드박스를 목표로 한다면, watershed 기반 named area는
장기적으로 중요한 시스템이 될 수 있다.

---

## 불변식

1. Voronoi edge는 hydrology substrate이지 자동 river가 아니다.
2. selected river segment는 descending 또는 outlet-carved graph logic을 따라야 한다.
3. local minima는 lake, sink, outlet carve 중 하나로 명시되어야 한다. local minimum이라는 이유만으로
   모두 lake가 되면 안 되며 dry/closed basin은 sink 또는 dry basin surface로 남을 수 있다.
4. selected river는 ocean outlet, 명시적인 lake/sink, 또는 downstream portal/outlet carve 없이 끊기면 안 된다.
5. selected river path는 generation order와 chunk order에 독립적이어야 한다.
6. Perlin 이후 micro depression은 macro river routing을 새로 정의하지 않는다.
7. river, lake, ocean, wetland는 같은 water mask로 뭉개지지 않고 의미가 구분되어야 한다.
8. final river morphology는 raw straight edge를 직접 terrain carve로 쓰지 않고, `river_plan`의
   reach morphology를 거쳐야 한다.
9. lake terminal/inlet river는 lake 면적/capacity에 비례해서 선택되어야 한다. 큰 lake는 더 큰
   raw inlet feeder를 요구하고 더 큰 selected/display discharge를 허용하지만, raw accumulation이 커도
   selected/display discharge는 ocean outlet river보다 보수적인 상한을 가져야 한다.
10. selected river는 lake 내부 edge를 관통하거나 lake boundary edge를 따라 스치지 않는다.
    lake와의 접촉은 land-side `LakeInlet`/`LakeOutlet` endpoint marker와 lake component pairing으로만
    표현하며, selected segment 자체는 lake corner를 endpoint로 삼지 않는다. selected segment가 쓰는
    edge의 `MacroLakeEdgeClass`도 반드시 `NonLake`여야 한다.
11. selected river graph의 shared corner는 confluence, branch, lake inlet/outlet, sink, coast outlet 중
    하나로 설명 가능해야 하며 독립 chain 교차는 허용하지 않는다.
12. preview-visible selected river graph는 ambiguous shared corner count가 0이어야 한다. 제거된 중복
    upstream branch 수는 `duplicate_trunk_pruned_count`에 기록해 튜닝 가능하게 유지한다.
13. 같은 selected chain은 lake contact를 최대 한 번만 가져야 한다. 이 정책으로 제거된 selected segment는
    `repeated_lake_contact_pruned_count`에 기록한다.
14. `disconnected_lake_inlet_count`, `disconnected_lake_outlet_count`,
    `selected_lake_edge_segment_count`는 정상 hydrology solve에서 0이어야 한다.
15. `unclassified_lake_connected_flow_count`는 정상 hydrology solve에서 0이어야 한다.

---

## 현재 구현 상태

- `src/world/generation/hydrology/mod.rs`가 `pub mod hydrology`로 연결되어 있다.
- `solve_hydrology`는 macro corner elevation, coast guide, graph corner adjacency를 읽어 downhill,
  graph-stage local minimum, outlet carve, watershed, flow accumulation, selected river segment를 만든다.
- local minimum은 `OceanOutlet`, `OutletCarve`, `Lake`, `Sink` 중 하나의 resolution으로 명시된다.
  launch 구현은 강 연속성을 우선해 ocean/coast까지 spill path가 있으면 outlet carve를 선택한다.
- selected river segment는 downstream chain을 따라 terminal outlet 또는 explicit sink/lake resolution까지
  이어지도록 선택된다.
- selected river selection은 ocean outlet chain과 lake terminal/inlet chain을 구분한다. lake
  terminal/inlet chain은 lake candidate footprint에서 산정한 capacity에 따라 lake별 top-N incoming
  chain, area-scaled inlet threshold, selected/display discharge cap을 적용한다. raw corner
  accumulation은 보존하고 segment의 `raw_flow_accumulation`에 기록한다. 기준을 통과한 lake-bound
  chain은 호수 직전 몇 edge로 truncate하지 않고, lake boundary 직전 land-side endpoint까지 이어질 수
  있다.
- selected river topology는 lake contact와 shared-corner intersection을 후처리로 검증한다. 결과 graph는
  selected segment endpoint에서만 생성되는 `LakeInlet`/`LakeOutlet` node와
  `GraphHydrologyTopologyStats`를 제공하며, preview와 테스트는 disconnected inlet/outlet, selected
  lake-edge river segment, invalid lake contact/intersection count와 ambiguous shared corner count가
  0인지 확인한다. disconnected count는 실제 selected segment incoming/outgoing map에서 marker endpoint를
  검사해 계산한다. 같은 corner에서
  겹쳐 보일 selected incoming은 가장 큰 flow branch만 남기고 나머지 upstream selected tree를 제거한다.
- `GraphDrainageNodeKind::Lake`는 selected river가 닿는 표시용 endpoint가 아니라, graph-stage local
  minimum이 lake resolution으로 남았음을 나타내는 내부 drainage/debug node다. 기본 preview에서는 이
  node를 그리지 않고, lake fill과 `LakeInlet`/`LakeOutlet` endpoint만 사용자가 보는 lake hydrology
  표면으로 취급한다.
- preview는 `macro_map_preview` composite 위에 selected river, lake/sink/outlet node를 overlay한다.
- 아직 구현되지 않은 것: lazy downstream portal의 region 간 persistence, lake water level solve,
  river plan realization, valley carve와 heightfield coupling.
