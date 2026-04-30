# macro_map

## 역할

`macro_map`은 graph base field를 해석해 대륙/바다/섬 ownership, resolved macro elevation,
산맥/능선/단층/해안 guide를 만드는 annotation layer다.

이 모듈의 출력은 최종 heightfield가 아니다. `macro_map`은 독자적인 continent/island noise source를
만들지 않고, graph stage가 제공한 smoothed `continentality`와 `elevation_seed`를 source of truth로
읽는다. 그런 다음 hydrology와 heightfield가 읽을 수 있는 장거리 ownership, elevation, ridge/coast
context를 resolve한다.

---

## 책임

- graph base `continentality`를 읽어 continent, ocean basin, island/archipelago ownership resolve
- ocean, continent, lake, wetland, coast 의미 구분을 위한 macro 입력 제공
- graph base `elevation_seed`, continentality, coast distance, basinness를 합성한 signed macro elevation resolve
- edge 기반 mountain/ridge/fault guide 선택
- land/ocean ownership 경계 기반 coast guide 선택
- ridge/fault/coast guide를 broad field로 확산
- hydrology가 읽을 drainage divide, basin, outlet 후보 제공

---

## 비책임

- 독자적인 continent/island noise source 생성
- river routing 확정. selected river chain, flow accumulation, outlet/lake/sink resolution은 hydrology 책임이다.
- noisy boundary curve 생성
- Perlin micro relief 합성
- final surface material 선택
- voxel fill

---

## 공개 API

현재 구현은 graph patch를 입력으로 받아 site/corner/edge annotation layer를 만든다. graph core에는
macro state를 직접 쓰지 않고, id 기반 별도 table을 반환한다.

```rust
MacroMapConfig::new(seed, generator_version) -> MacroMapConfig
generate_macro_map(&VoronoiGraphPatch, MacroMapConfig) -> GraphMacroMap

GraphMacroMap {
    sites: Vec<MacroSite>,
    corners: Vec<MacroCorner>,
    edges: Vec<MacroEdge>,
}

MacroSurfaceKind::{
    Continent,
    Island,
    OceanBasin,
    CoastLand,
    CoastIsland,
    CoastOcean,
    LakeCandidate,
    WetlandCandidate,
}
MacroEdgeGuide {
    is_coast,
    is_mountain_candidate,
    is_ridge_candidate,
    is_river_candidate,
    is_fault_candidate,
    coastness,
    mountainness,
    ridgeness,
    signed_elevation_gradient,
    drainage_divide_potential,
    river_potential,
}
```

`MacroMapConfig`는 land/ocean 비율을 진단하고 조율하기 위한 공개 tuning handle을 가진다.
`land_bias`는 continent/ocean ownership 합성값에 더해지는 signed offset이며, 양수일수록 land
ownership이 늘고 음수일수록 ocean basin ownership이 늘어난다.

이전 transition 구현에 있던 `island_strength`와 super-cell continent/island source는 제거한다.
island/archipelago 성향은 graph base `continentality`의 양수 component 해석에서만 나온다.

`MacroSite`는 continent/ocean basin id, signed macro elevation, continentality,
coastness/distance-to-coast, mountainness, ridgeness, basinness를 가진다. 작은 land component는
`Island` 또는 `CoastIsland` surface kind로 드러나며, 별도 island noise source에서 만들어지지 않는다.
`MacroCorner`는 인접 site ownership과 corner base field를 읽어 같은 macro field를 샘플한다.
`MacroEdge`는 두 site의 macro ownership과 elevation context를 읽어 coast/ridge/fault guide를 붙인다.
stage 3 macro_map은 river corridor를 선택하지 않는다. selected river chain, flow accumulation,
lake/sink/outlet carve는 hydrology 단계가 확정한다.

---

## Continent, Ocean, Lake, Coast

water는 단순히 `height < sea_level`로 끝내면 안 된다. world는 물의 의미를 구분해야 한다.

이 프로젝트는 Amit의 작은 island map과 달리 큰 대륙과 큰 바다가 공존하는 구조를 목표로 한다.
대륙은 하나의 작은 섬이 아니라 장거리 macro ownership을 가진 land mass이며, 그 내부에 산맥,
분수계, 강, 호수, 습지, 평야, 해안 지형이 배치된다.

- ocean: 큰 바다 또는 외부 ocean basin과 연결된 물
- continent: 큰 land mass와 그 내부 macro elevation / drainage ownership
- island / archipelago: graph base `continentality`가 만든 land component 중 큰 continent에 속하지
  않는 작은 양수 component
- lake: land 내부의 local minimum 또는 basin fill로 생긴 고립 물
- wetland/marsh: 얕은 물, 높은 hydration, 낮은 slope가 겹친 지역
- coast: ocean과 land 사이의 transition band
- beach/cliff/rocky shore: coast의 slope, exposure, material policy에 따른 표면 표현

Amit의 island map에서는 border flood fill로 ocean과 lake를 구분할 수 있지만, 이 프로젝트는
무한 월드이므로 같은 방법을 그대로 쓸 수 없다. 대신 graph scale의 coherent `continentality`,
component ownership, ocean basin classification, outlet-to-ocean routing을 사용해야 한다.

Land/ocean 판정은 아래 입력을 합성하되, source of truth는 graph base field다.

- graph base `continentality`
- graph base `elevation_seed`
- connected land/ocean component
- component size와 ocean basin 연결성
- coast distance / coastness
- sea-level contract
- local lake/sink resolution

`target land ratio`만으로는 대륙성이 보장되지 않는다. land ratio는 preview area 또는 graph patch에서
land/open water 비율을 조율하는 보조 tuning일 뿐이다. 대륙성은 graph base `continentality`가
장거리 coherent field를 제공하고, macro_map이 connected component policy로 해석해야 보장된다.

launch 정책은 아래처럼 잡는다.

- graph base field stage가 대륙성/해양성 site가 뭉치는 `continentality`를 먼저 만든다.
- macro_map은 `continentality >= threshold`를 초기 land mask로 보고 connected component를 resolve한다.
- 큰 land component는 continent, ocean basin 안의 작은 land component는 island 또는 archipelago로 분류한다.
- signed macro elevation은 graph `elevation_seed`, `continentality`, coast distance, basinness를
  합성하며, sign 하나만으로 대륙/바다 의미를 결정하지 않는다.
- 작은 양수 land component는 기본적으로 island 또는 archipelago candidate다.
- launch 기본값에서도 큰 대륙만 만들지 않고, graph `continentality`가 ocean basin 안에 크고 작은
  양수 component를 만들 수 있어야 한다.
- 섬은 macro_map이 대륙 ownership을 뒤집어 만든 예외가 아니라 graph `continentality`의 component
  해석 결과여야 한다.
- 이후 hydrology/surface 단계에서는 큰 대륙에 붙지 않은 land component를 island로 취급하고, 최소 크기, 해안 폭, 담수 생성 가능성, 식생 밀도 정책을 다르게 줄 수 있어야 한다.

무한 월드에서는 전체 land cell 수와 ocean cell 수를 전역으로 세어 제약할 수 없다. 대신
deterministic graph base field, 충분한 padding, component pruning/assimilation 규칙, border portal
계약으로 요청 영역마다 같은 대륙성이 재현되게 만든다.

현재 launch 구현은 graph base `continentality`를 그대로 읽어 land/ocean ownership을 판정한다.
`land_bias`와 `sea_level`은 그 값에 적용되는 signed offset일 뿐이며, macro_map은 별도 continent/island
noise source를 합성하지 않는다. signed macro elevation은 graph base `elevation_seed`,
`continentality`, coastness, basinness를 합성해 얻는다. component id와 distance 값은 아직 launch
scaffold 수준의 deterministic hint이며, 이후 connected component resolve로 대체되어야 한다.

---

## Macro Elevation

macro elevation은 대륙, 바다, 산맥, 능선, 분수계 context를 이미 알고 있는 graph-derived
field다. Perlin noise는 이 macro structure를 뒤집는 source가 아니라, 마지막 표면에 국소적인
높낮이와 질감을 더하는 micro relief다.

macro elevation resolve 순서:

1. graph base `continentality`를 land/ocean mask로 해석한다.
2. connected component를 resolve해 continent, ocean basin, island/archipelago ownership을 정한다.
3. graph base `elevation_seed`, `continentality`, coast distance, basinness를 합성한다.
4. signed macro elevation을 만들되, ownership과 sea level contract를 함께 저장한다.
5. edge 기반 mountain/ridge/fault/plateau 후보를 먼저 정한다.
6. coast는 signed macro elevation 경계가 아니라 land ownership과 connected-ocean basin 경계에서 우선 찾는다.
7. ridge/fault/coast skeleton을 broad field로 확산한다.
8. hydrology가 사용할 divide, basin, outlet 후보를 annotation한다.

---

## Mountain And Ridge Structure

산맥은 단순히 높은 noise가 아니다.

world는 graph 위에 mountain belt / ridge chain / fault line 후보를 소유해야 한다. 단순히 high
elevation edge만 고르면 높은 평원도 ridge가 되어버린다. ridge는 높은 값뿐 아니라 주변 gradient,
연속 chain, drainage divide, uplift/fault 성격을 함께 만족해야 한다.

구현 방식은 아래 중 하나 또는 조합이 될 수 있다.

- graph site chain을 mountain belt로 선택
- edge chain을 fault/ridge candidate로 선택
- plate-like region boundary를 uplift source로 사용
- land component 내부 위치와 coast distance를 이용해 broad mountainness field 생성
- ruggedness와 elevation bias로 ridge 주변 local relief 강화

현재 launch 구현의 mountain/ridge/fault 후보는 같은 land component 내부 edge만 대상으로 한다.
단순히 signed macro elevation이 높은 두 site를 잇는 edge는 ridge가 아니다. edge guide는 두 site의
signed macro elevation gradient, inlandness/coast distance, mountainness/ridgeness, 낮은 basinness,
drainage divide 가능성을 함께 점수화한다.

- mountain candidate는 같은 land component 내부의 inland highland envelope다.
- ridge candidate는 mountain candidate 중 elevation gradient와 drainage divide potential을 함께
  만족하는 skeleton이다.
- fault candidate는 같은 land component 내부에서 signed elevation gradient가 크고 산악성이 있는
  edge다.

이 단계의 ridge/fault/mountain은 최종 능선 mesh가 아니라 hydrology와 Voronoi-derived macro field가
읽을 skeleton이다.

산맥은 hydrology보다 먼저 정해져야 한다. 대륙 내부의 큰 산맥과 ridge는 분수계와 강의 방향을
만드는 원인이며, hydrology가 나중에 그 구조를 읽어야 한다.

산맥은 macro elevation과 Voronoi-derived noise map에 아래 방식으로 반영한다.

- distance-to-ridge gradient
- along-ridge variation
- pass/saddle lowering
- drainage divide ownership
- snow/alpine temperature modifier
- erosion/valley carve에 대한 저항 또는 우선순위

산맥 선이 그대로 보이면 안 된다. ridge skeleton은 broad envelope로 확산되고, noisy boundary와
Voronoi-derived gradient map을 거쳐 자연스러운 능선/봉우리/안부로 바뀌어야 한다. Perlin micro
relief는 마지막에 이 구조 위에 얹히는 표면 디테일이다.

---

## Impassable Or Discontinuous Borders

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

이 edge들은 visible boundary가 될 수 있지만, raw polygon edge가 그대로 보이면 안 된다.
noisy boundary, local erosion, talus/sediment, vegetation mask를 통해 자연스럽게 현실화해야 한다.

---

## 불변식

1. 대륙/바다 ownership은 chunk 생성 순서와 독립적이어야 한다.
2. 대륙성은 target land ratio가 아니라 graph base `continentality`의 coherence와 connected component 정책으로 보장한다.
3. macro elevation은 Perlin micro relief보다 먼저 계산되어야 한다.
4. mountain/ridge/fault/coast guide는 hydrology보다 먼저 결정되어야 한다.
5. macro_map은 독자적인 continent/island noise source를 만들지 않고 graph base field를 resolve해야 한다.
6. coast guide는 connected ocean basin과 land ownership의 경계를 우선한다.
7. graph-derived mountain/ridge/coast guide는 broad field로 확산되어야 하며 raw segment가 그대로 보이면 안 된다.
8. selected river chain과 outlet/lake/sink resolution은 hydrology가 확정한다.
9. ocean, lake, wetland, coast의 의미 구분은 surface policy와 preview에서 유지되어야 한다.

---

## 현재 구현 상태

- `src/world/generation/macro_map/mod.rs`가 `pub mod macro_map`으로 연결되어 있다.
- `generate_macro_map`은 rayon으로 site/corner/edge annotation을 병렬 생성하고, id 정렬로 deterministic order를 유지한다.
- continent/ocean ownership은 graph base `continentality`를 source of truth로 읽고, `land_bias`와
  `sea_level` offset만 적용해 정한다. macro_map은 독자적인 continent/island noise source를 만들지 않는다.
- signed macro elevation은 land 양수, ocean 음수 contract를 유지한다.
- site/corner annotation은 coastness, distance-ish coast value, mountainness, ridgeness, basinness를 포함한다.
- edge guide는 coast, mountain candidate, ridge candidate, fault candidate를 포함한다. ridge는
  단순 high elevation edge가 아니라 같은 land component 내부성, signed elevation gradient,
  inlandness, mountain/rugged context, drainage divide potential을 함께 만족해야 한다.
  stage 3 macro_map은 hydrology 전 river candidate corridor를 선택하지 않으며, selected river chain,
  flow accumulation, lake/sink/outlet carve는 hydrology stage가 확정한다.
