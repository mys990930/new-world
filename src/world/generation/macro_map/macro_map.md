# macro_map

## 역할

`macro_map`은 graph base field를 해석해 대륙/바다/섬 ownership, resolved macro elevation,
능선/단층/해안 guide를 만드는 annotation layer다.

이 모듈의 출력은 최종 heightfield가 아니다. `macro_map`은 독자적인 continent/island noise source를
만들지 않고, graph stage가 제공한 smoothed `continentality`와 `elevation_seed`를 source of truth로
읽는다. 그런 다음 hydrology와 heightfield가 읽을 수 있는 장거리 ownership, elevation, ridge/coast
context를 resolve한다.

---

## 책임

- graph base `continentality`를 읽어 continent, ocean basin, island/archipelago ownership resolve
- ocean, continent, lake, wetland, coast 의미 구분을 위한 macro 입력 제공
- graph base `elevation_seed`, continentality, basinness를 합성한 signed macro elevation resolve
- connected ocean coast에 인접한 land owner는 waterline-compatible한 낮은 양수 elevation으로 제한하되, 그 바깥 land elevation은 graph elevation/ruggedness/mountain/basin context를 그대로 보존한다. launch slice의 coastal ceiling은 sea level 바로 위의 아주 작은 band에 머물러야 하며, 높은 terrace를 만들지 않는다.
- edge 기반 ridge/fault guide 선택. mountainness/rugged context는 public edge guide가 아니라 점수 입력이다.
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
    biomes: Vec<GraphBiomeCell>,
}

MacroSurfaceKind::{
    Continent,
    Island,
    OceanBasin,
    CoastLand,
    CoastIsland,
    CoastOcean,
    DryBasin,
    LakeCandidate,
    WetlandCandidate,
}
MacroEdgeGuide {
    is_coast,
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
MacroLakeEdgeClass::{
    NonLake,
    LakeAdjacentLand,
    LakeBoundary,
    LakeInternal,
}

GraphBiomeKind::{
    ShallowOcean,
    DeepOcean,
    Coast,
    Lake,
    Wetland,
    DryBasin,
    ...
}
```

`MacroMapConfig`는 land/ocean 비율을 진단하고 조율하기 위한 공개 tuning handle을 가진다.
`land_bias`는 continent/ocean ownership 합성값에 더해지는 signed offset이며, 양수일수록 land
ownership이 늘고 음수일수록 ocean basin ownership이 늘어난다.
launch 기본값은 `DEFAULT_MACRO_LAND_BIAS = 0.14`이며, 기본 preview window에서 대략
land:water = 6:4에 가까운 비율을 목표로 한다. 이 값은 graph base `continentality`의 coherent
field를 새로 만들지 않고 threshold를 이동하는 tuning handle이다. 따라서 coastline 복잡도는
여전히 graph continentality/elevation field의 장거리 등고선과 connected component resolve가 만든다.

이전 transition 구현에 있던 `island_strength`와 super-cell continent/island source는 제거한다.
island/archipelago 성향은 graph base `continentality`의 양수 component 해석에서만 나온다.

`MacroSite`는 continent/ocean basin id, signed macro elevation, continentality,
coastness/distance-to-coast, mountainness, ridgeness, basinness를 가진다. launch 구현의
`distance_to_coast_blocks`는 coast guide와 surface context용 annotation이다. positive land elevation을
coast distance에 따라 계속 낮췄다가 inland에서 회복시키는 source가 되어서는 안 된다. 다만
coast-adjacent owner 자체는 바다 terrain과 연속적으로 만날 수 있도록 sea level 바로 위의 아주 작은 양수
ceiling을 갖고, 이 ceiling은 ruggedness/mountainness/ridgeness가 강할수록 약간 높아진다.
작은 land component는
`Island` 또는 `CoastIsland` surface kind로 드러나며, 별도 island noise source에서 만들어지지 않는다.
`MacroCorner`는 인접 site ownership과 corner base field를 읽어 같은 macro field를 샘플한다.
`MacroEdge`는 두 site의 macro ownership과 elevation context를 읽어 coast/ridge/fault guide를 붙인다.
또한 edge의 인접 site surface kind와 양 endpoint corner surface kind를 함께 읽어 `MacroLakeEdgeClass`를
붙인다. 이 class는 preview nearest-site fill에서 보이는 lake edge와 hydrology가 금지하는 edge가
서로 다른 기준을 보지 않도록 맞추기 위한 명시적 edge annotation이다.
stage 3 macro_map은 river corridor를 선택하지 않는다. selected river chain, flow accumulation,
lake/sink/outlet carve는 hydrology 단계가 확정한다.
현재 구현에서도 `MacroEdgeGuide.is_river_candidate`는 selected river 의미로 사용하지 않는다.
selected river는 `hydrology::solve_hydrology`의 `GraphRiverSegment`만 source of truth다.
`GraphMacroMap.biomes`는 site id별 final cell biome context와 classification을 가진다. 이
classification은 macro_map 끝에서 생성되어 macro_field가 nearest site의 biome 의미를 함께 전달할 수
있게 한다. Oceanic은 단일 biome으로 남기지 않고 `ShallowOcean`과 `DeepOcean`으로 분리한다.
macro_map이 만드는 biome context는 hydrology 이전 base pass이며, selected hydrology가 풀린 뒤
`apply_headwater_source_hydration_to_biomes`가 수원지 근처 land/dry-basin cell을 다시 보정할 수 있다.
수원지 근처는 `GraphHydrologyRole::Headwater` segment의 edge 양쪽 site로 정의하고, final hydration
floor는 `0.46`이다. 이 pass는 macro ownership/surface kind를 바꾸지 않고 `GraphBiomeCell.context`와
`GraphBiomeCell.biome`만 갱신한다.
`MacroSurfaceKind::LakeCandidate`는 biome lake 판정의 source of truth다. 따라서 site가
`LakeCandidate`이면 같은 site의 `GraphBiomeCell.context.water_role`은 반드시 `Lake`이고,
`GraphBiomeCell.biome`은 반드시 `GraphBiomeKind::Lake`여야 한다. 반대로 `LakeCandidate`가 아닌
site가 biome lake로 승격되면 macro/biome preview가 서로 다른 호수 mask를 보게 되므로 회귀다.
land biome classification은 graph temperature/hydration, signed macro elevation, continentality,
coastness, mountainness, ridgeness-derived ruggedness를 읽되 water/coast/lake/wetland/dry-basin role이 climate-only class보다
우선한다. Temperate grassland와 hot dry/wet tropical classes가 사라지지 않도록 bounded seed
distribution test로 확인한다.

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
- lake: land 내부의 local minimum, basin fill, 또는 open ocean component와 연결되지 않은 내륙 물
- wetland/marsh: 얕은 물, 높은 hydration, 낮은 slope가 겹친 지역
- dry basin / closed basin: 내륙 저지대지만 지속 수면을 만들 만큼 깊거나 습하지 않은 폐쇄분지
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
- explicit ocean basin distance / coastness
- sea-level contract
- local lake/sink resolution

`target land ratio`만으로는 대륙성이 보장되지 않는다. land ratio는 preview area 또는 graph patch에서
land/open water 비율을 조율하는 보조 tuning일 뿐이다. 대륙성은 graph base `continentality`가
장거리 coherent field를 제공하고, macro_map이 connected component policy로 해석해야 보장된다.

launch 정책은 아래처럼 잡는다.

- graph base field stage가 대륙성/해양성 site가 뭉치는 `continentality`를 먼저 만든다.
- macro_map은 `continentality >= threshold`를 초기 land mask로 보고 connected component를 resolve한다.
- 큰 land component는 continent, ocean basin 안의 작은 land component는 island 또는 archipelago로 분류한다.
- 음수 `continentality` water component라도 explicit ocean basin과 연결되지 않으면 바다에 가까워 보여도
  `OceanBasin`이 아니라 `LakeCandidate`, `WetlandCandidate`, 또는 `DryBasin`으로 분류한다.
- patch/open boundary 또는 guard/padding boundary에 닿는다는 사실만으로 ocean이 되면 안 된다.
  launch 구현은 가장 큰 장거리 water component와 충분히 큰/충분히 oceanic한 secondary component만
  explicit ocean basin으로 보고, 나머지 고립 water component는 lake/wetland 후보로 유지한다.
- 고립 water component가 모두 lake가 되면 안 된다. launch 정책은 site/cell 기준 10개 이하의 작은
  component를 일반 lake 후보로 보고, 30개 안팎의 큰 lake는 component hash와 깊은/습한 basin 조건이
  동시에 맞을 때만 드물게 허용한다. 그 외 큰 폐쇄 저지대는 wetland 또는 dry basin으로 흡수한다.
  이 값은 launch tuning용 soft cap이며, 이후 heightfield/water level solve가 들어오면 component
  내부 일부만 수면으로 남기는 방식으로 더 정교화한다.
- 1~3 site/cell 규모의 작은 tiny local-minima lake는 큰 호수 억제 정책과 별개로 낮은 확률로 허용한다.
  이 후보는 river-side일 필요가 없다. macro_map은 graph adjacency에서 더 낮은 land neighbor가 없는
  land-owned local-minima component를 찾고, component size가 1~3 cell이며 낮은 `elevation_seed`,
  충분한 hydration, ocean coast에서 떨어진 위치, deterministic component roll을 통과할 때만
  `LakeCandidate`로 승격한다. launch 기본 확률은 `DEFAULT_TINY_LOCAL_MINIMA_LAKE_CHANCE_PER_10K =
  3600`이며, 좋은 score의 후보도 `DEFAULT_TINY_LOCAL_MINIMA_LAKE_MAX_CHANCE_PER_10K = 4000` 상한을
  넘지 않는다. 강줄기 중간 또는 독립 폐쇄 저지대의 작은 물웅덩이를 이전보다 자주 만들되 모든 local
  minimum을 물로 채우지 않고 dry basin / closed basin 표현을 계속 유지하는 것이 목적이다.
- signed macro elevation은 graph `elevation_seed`, `continentality`, basinness를 합성하며, sign 하나만으로
  대륙/바다 의미를 결정하지 않는다. positive land elevation은 coast distance를 장거리 단조 상승 축으로
  사용하지 않는다. shoreline owner는 waterline-compatible ceiling으로 제한하지만, 그 다음 land는
  coast distance recovery curve가 아니라 graph elevation, ruggedness, mountainness, basinness가 만든
  값으로 이어져야 한다. macro_field는 CoastLand owner를 주변 highland scalar 평균으로 다시 끌어올리면 안 된다.
- land signed macro elevation은 coast-adjacent cell에서 곧바로 full highland 값으로 뛰면 안 된다.
  일부 해안은 lowland라 완만할 수 있고, 일부 해안은 high/rugged context라 더 가파를 수 있다. 모든 coast를
  같은 uniform coast-distance ramp나 같은 cliff foot으로 만들면 회귀다. 내륙 전체를 coast distance에
  따라 계속 올리는 S-curve가 되면 회귀다. 내륙 고도는 내려갔다 올라갈 수 있어야 하고, lake/wetland
  승격에 쓰는 local-minima 판단을 인위적으로 늘리면 안 된다.
- 작은 양수 land component는 기본적으로 island 또는 archipelago candidate다.
- launch 기본값에서도 큰 대륙만 만들지 않고, graph `continentality`가 ocean basin 안에 크고 작은
  양수 component를 만들 수 있어야 한다.
- 섬은 macro_map이 대륙 ownership을 뒤집어 만든 예외가 아니라 graph `continentality`의 component
  해석 결과여야 한다.
- 이후 hydrology/surface 단계에서는 큰 대륙에 붙지 않은 land component를 island로 취급하고, 최소 크기, 해안 폭, 담수 생성 가능성, 식생 밀도 정책을 다르게 줄 수 있어야 한다.

closed inland basin은 단일 `DryBasin` bucket으로 몰아넣지 않는다. 깊거나 작은 물 component와
tiny local-minima lake는 `LakeCandidate`, 습하고 완만한 폐쇄 저지대는 `WetlandCandidate`, 지속
수면 조건이 약한 큰 폐쇄 저지대는 `DryBasin`으로 남긴다. 이 구분은 biome water role까지 전달되어
surface/material 단계에서 서로 다른 정책을 적용할 수 있어야 한다.

무한 월드에서는 전체 land cell 수와 ocean cell 수를 전역으로 세어 제약할 수 없다. 대신
deterministic graph base field, 충분한 padding, component pruning/assimilation 규칙, border portal
계약으로 요청 영역마다 같은 대륙성이 재현되게 만든다.

현재 launch 구현은 graph base `continentality`를 그대로 읽어 land/ocean ownership을 판정한다.
`land_bias`와 `sea_level`은 그 값에 적용되는 signed offset일 뿐이며, macro_map은 별도 continent/island
noise source를 합성하지 않는다. 기본 `land_bias`는 6:4 land/water preview target을 위한 조율점이고,
CLI preview에서는 `--land-bias`로 override할 수 있다. signed macro elevation은 graph base `elevation_seed`,
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
3. graph base `elevation_seed`, `continentality`, basinness를 합성한다.
4. signed macro elevation을 만들되, connected ocean coast-adjacent owner는 sea level 바로 위의 작은 양수
   ceiling으로 제한한다. 그 바깥 positive land height는 coast distance recovery가 아니라 graph
   elevation/ruggedness/mountain/basin context를 따른다.
   ownership과 sea level contract는 함께 저장한다.
5. edge 기반 ridge/fault/plateau 후보를 먼저 정한다.
6. coast는 signed macro elevation 경계가 아니라 connected ocean basin과 non-ocean terrain 경계에서 우선 찾는다.
   내륙 lake/wetland/dry basin과 주변 land의 경계는 coast가 아니다.
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

현재 launch 구현의 ridge/fault 후보는 같은 land component 내부 edge만 대상으로 한다.
단순히 signed macro elevation이 높은 두 site를 잇는 edge는 ridge가 아니다. edge guide는 두 site의
signed macro elevation gradient, inlandness/coast distance, mountainness/ridgeness, 낮은 basinness,
drainage divide 가능성을 함께 점수화한다.

- mountainness는 같은 land component 내부의 inland highland/rugged envelope를 나타내는 scalar context다.
- ridge candidate는 mountainness/rugged context, signed elevation gradient, inlandness, 낮은 basinness,
  drainage divide potential을 함께 만족하는 public edge guide다.
- fault candidate는 같은 land component 내부에서 signed elevation gradient가 크고 산악성이 있는
  edge다.

이 단계의 ridge/fault guide는 최종 능선 mesh가 아니라 hydrology와 Voronoi-derived macro field가
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
macro_field 단계의 ridge influence는 이 skeleton을 heightfield가 읽을 수 있는 연결된 산맥 envelope로
해석한다. 단일 edge pixel만 밝은 pinpoint로 남기면 안 되며, selected ridge chain을 따라 폭 있는
mountain belt shoulder가 이어져야 한다. 동시에 ridge tail이 전역에 깔려 micro noise처럼 보이는 것도
회귀다.

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
4. ridge/fault/coast guide는 hydrology보다 먼저 결정되어야 한다.
5. macro_map은 독자적인 continent/island noise source를 만들지 않고 graph base field를 resolve해야 한다.
6. coast guide는 connected ocean basin과 land ownership의 경계를 우선한다.
7. 내륙 water component는 signed elevation이 음수여도 connected ocean basin이 아니면 lake/wetland/dry basin
   후보로 유지해야 한다.
8. lake edge는 site/corner 혼합 판정이 아니라 `MacroLakeEdgeClass`로 명시되어야 한다. hydrology가
   selected river를 금지할 때도 이 edge class를 읽어야 한다.
9. graph-derived ridge/fault/coast guide는 broad field로 확산되어야 하며 raw segment가 그대로 보이면 안 된다.
10. selected river chain과 outlet/lake/sink resolution은 hydrology가 확정한다.
11. ocean, lake, wetland, coast의 의미 구분은 surface policy와 preview에서 유지되어야 한다.

---

## 현재 구현 상태

- `src/world/generation/macro_map/mod.rs`가 `pub mod macro_map`으로 연결되어 있다.
- `generate_macro_map`은 rayon으로 site/corner/edge annotation을 병렬 생성하고, id 정렬로 deterministic order를 유지한다.
- continent/ocean ownership은 graph base `continentality`를 source of truth로 읽고, `land_bias`와
  `sea_level` offset만 적용해 정한다. launch 기본 `land_bias`는 0.14로, 기본 preview window에서
  대략 6:4 land/water를 목표로 한다. macro_map은 독자적인 continent/island noise source를 만들지 않는다.
- ocean/lake ownership은 water component connectivity를 함께 읽는다. patch/open boundary에 연결된다는
  사실만으로 ocean이 되지는 않으며, explicit ocean basin으로 분류되지 않은 고립 water component는
  lake candidate로 surface kind를 바꾼다.
- 큰 lake는 드문 deep/wet basin 조건으로 제한하고, 1~3 site/cell tiny local-minima lake는 land-owned
  저지대에서 graph local-minima component 판정과 deterministic low-probability 조건을 통과할 때만
  추가한다. 이 작은 lake는 river-side에 붙어 있을 필요가 없다.
- signed macro elevation은 land 양수, ocean 음수 contract를 유지한다.
- site/corner annotation은 explicit ocean-coast 기준 coastness, distance-ish coast value, mountainness, ridgeness, basinness를 포함한다.
- `mountainness`와 `ridgeness`는 graph의 coherent smoothed ruggedness를 입력으로 읽는다. 단일 site hash
  ruggedness를 그대로 쓰지 않으므로 인접 macro cell 사이의 산악/평원 전환이 덜 discrete하다.
- edge guide는 coast, ridge candidate, fault candidate를 포함한다. ridge는
  단순 high elevation edge가 아니라 같은 land component 내부성, signed elevation gradient,
  inlandness, mountainness/rugged context, drainage divide potential을 함께 만족해야 한다.
  stage 3 macro_map은 hydrology 전 river candidate corridor를 선택하지 않으며, selected river chain,
  flow accumulation, lake/sink/outlet carve는 hydrology stage가 확정한다.
- edge annotation은 `MacroLakeEdgeClass`를 포함한다. `LakeInternal`, `LakeBoundary`,
  `LakeAdjacentLand`는 selected river segment가 사용할 수 없는 edge class다.
- hydrology 구현은 macro_map의 signed elevation, coast guide, ridge/fault context를 입력으로 읽지만,
  macro_map이 river 연속성이나 outlet 정책을 소유하지는 않는다.
