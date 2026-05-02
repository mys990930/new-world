# hydrology

## 역할

`hydrology`는 Voronoi corner와 selected edge를 기반으로 한 graph-first 물 흐름 계약을 소유한다.

polygon 경계는 river path가 될 수 있는 graph substrate지만, 모든 경계가 강이 되어서는 안 된다.
hydrology는 macro_map이 graph base field에서 resolve한 ownership/elevation, ridge, coast, basin
정보를 읽어 downhill routing, watershed, selected river segment, lake/outlet 처리를 계산하고,
이후 heightfield가 valley와 water surface를 알 수 있게 제약을 제공한다.

---

## 책임

- watershed, drainage node, river segment 표현
- selected graph edge를 headwater, tributary, trunk, floodplain, outlet으로 분류
- flow accumulation과 downstream progress 유지
- lake, sink, outlet carve 같은 local minima 처리 계약 정의
- heightfield와 surface plan이 읽을 valley/water constraint 제공

---

## 비책임

- base noise heightfield 생성
- final voxel channel carve
- sediment 또는 surface material 선택
- noisy river spline curve 생성
- live world storage mutation

---

## Hydrology Graph

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
```

`GraphRiverSegment`는 potential guide가 아니라 selected river result다. preview와 이후
heightfield는 이 segment만 강으로 해석해야 한다. macro_map의 ridge/fault/coast guide는 이 단계의
입력일 뿐이며, pre-hydrology river candidate와 혼동하면 안 된다.

---

## 처리 순서

1. macro_map의 resolved ownership/elevation, ridge guide, coast guide를 읽는다.
2. graph corner elevation을 계산한다.
3. downhill edge를 고른다.
4. graph-stage local minimum을 찾는다.
5. local minimum을 lake로 유지할지, sink로 둘지, outlet을 carve할지 결정한다.
6. watershed와 flow accumulation을 계산한다.
7. 충분한 flow와 지형 조건을 만족하는 edge chain만 selected river로 선택한다.
8. selected river chain이 ocean outlet, 명시적인 lake/sink, 또는 downstream portal/outlet carve 없이 끊기지 않도록 검증한다.
9. final heightfield가 river corridor를 알고 생성되도록 valley constraint를 제공한다.

launch 구현은 아래의 보수적인 정책을 사용한다.

- terminal outlet은 ocean/coast corner 또는 coast guide와 인접한 corner다.
- 일반 corner는 인접 corner 중 더 낮은 elevation 또는 ocean/coast terminal을 downhill target으로 고른다.
- 더 낮은 이웃이 없는 graph-stage local minimum은 spill path search를 수행한다.
- spill path가 ocean/coast terminal까지 닿으면 outlet carve로 downstream edge chain을 만든다.
- spill path가 없으면 explicit sink로 남긴다. selected river는 explicit sink를 제외하고 중간에서 끊기면 안 된다.
- flow accumulation은 land corner rainfall contribution을 downstream으로 누적한다.
- selected river는 threshold를 넘은 headwater에서 시작하되, 선택된 순간 downstream chain을 outlet/sink/lake까지 계속 포함한다.

최종 river geometry는 raw edge segment가 아니다.

- edge chain을 spline으로 잇는다.
- edge guard quadrilateral 안에서 noisy line을 만든다.
- river width, floodplain, gravel bar, wetland는 flow와 local slope에 따라 조절한다.
- confluence는 각진 snapping이 보이지 않도록 downstream smoothing을 적용한다.

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
3. local minima는 lake, sink, outlet carve 중 하나로 명시되어야 한다.
4. selected river는 ocean outlet, 명시적인 lake/sink, 또는 downstream portal/outlet carve 없이 끊기면 안 된다.
5. selected river path는 generation order와 chunk order에 독립적이어야 한다.
6. Perlin 이후 micro depression은 macro river routing을 새로 정의하지 않는다.
7. river, lake, ocean, wetland는 같은 water mask로 뭉개지지 않고 의미가 구분되어야 한다.
8. final river geometry는 raw straight edge가 아니라 spline/domain-warped realization을 사용해야 한다.

---

## 현재 구현 상태

- `src/world/generation/hydrology/mod.rs`가 `pub mod hydrology`로 연결되어 있다.
- `solve_hydrology`는 macro corner elevation, coast guide, graph corner adjacency를 읽어 downhill,
  graph-stage local minimum, outlet carve, watershed, flow accumulation, selected river segment를 만든다.
- local minimum은 `OceanOutlet`, `OutletCarve`, `Lake`, `Sink` 중 하나의 resolution으로 명시된다.
  launch 구현은 강 연속성을 우선해 ocean/coast까지 spill path가 있으면 outlet carve를 선택한다.
- selected river segment는 downstream chain을 따라 terminal outlet 또는 explicit sink/lake resolution까지
  이어지도록 선택된다.
- preview는 `macro_map_preview` composite 위에 selected river, lake/sink/outlet node를 overlay한다.
- 아직 구현되지 않은 것: lazy downstream portal의 region 간 persistence, lake water level solve,
  noisy river spline realization, valley carve와 heightfield coupling.
