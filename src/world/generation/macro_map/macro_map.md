# macro_map

## 역할

`macro_map`은 대륙/바다 ownership, Voronoi 기반 macro elevation, 산맥/능선/단층/해안 guide를
소유한다.

이 모듈의 출력은 최종 heightfield가 아니다. hydrology와 heightfield가 읽을 수 있는 장거리 구조와
gradient map을 만든다.

---

## 책임

- continent와 ocean basin ownership 정의
- ocean, continent, lake, wetland, coast 의미 구분을 위한 macro 입력 제공
- Voronoi graph 기반 macro elevation 생성
- edge 기반 mountain/ridge/fault/coast/river-candidate guide 선택
- ridge/fault/coast guide를 broad field로 확산
- hydrology가 읽을 drainage divide, basin, outlet 후보 제공

---

## 비책임

- river routing 확정. `macro_map`의 river는 hydrology 전 단계 후보 annotation일 뿐이다.
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

MacroSurfaceKind::{Continent, OceanBasin, CoastLand, CoastOcean, LakeCandidate, WetlandCandidate}
MacroEdgeGuide {
    is_coast,
    is_ridge_candidate,
    is_river_candidate,
    is_fault_candidate,
    coastness,
    ridgeness,
    river_potential,
}
```

`MacroSite`는 continent/ocean basin id, signed macro elevation, continentality,
coastness/distance-to-coast, mountainness, ridgeness, basinness를 가진다. `MacroCorner`는 corner
position에서 같은 macro field를 샘플한다. `MacroEdge`는 두 site의 macro ownership과 elevation
context를 읽어 hydrology 이전 guide를 붙인다.

---

## Continent, Ocean, Lake, Coast

water는 단순히 `height < sea_level`로 끝내면 안 된다. world는 물의 의미를 구분해야 한다.

이 프로젝트는 Amit의 작은 island map과 달리 큰 대륙과 큰 바다가 공존하는 구조를 목표로 한다.
대륙은 하나의 작은 섬이 아니라 장거리 macro ownership을 가진 land mass이며, 그 내부에 산맥,
분수계, 강, 호수, 습지, 평야, 해안 지형이 배치된다.

- ocean: 큰 바다 또는 외부 ocean basin과 연결된 물
- continent: 큰 land mass와 그 내부 macro elevation / drainage ownership
- island / archipelago: ocean basin 안에서 별도 island field가 만든 크고 작은 양수 land component
- lake: land 내부의 local minimum 또는 basin fill로 생긴 고립 물
- wetland/marsh: 얕은 물, 높은 hydration, 낮은 slope가 겹친 지역
- coast: ocean과 land 사이의 transition band
- beach/cliff/rocky shore: coast의 slope, exposure, material policy에 따른 표면 표현

Amit의 island map에서는 border flood fill로 ocean과 lake를 구분할 수 있지만, 이 프로젝트는
무한 월드이므로 같은 방법을 그대로 쓸 수 없다. 대신 graph scale의 ocean basin ownership,
continent ownership, continentality field, outlet-to-ocean routing을 사용해야 한다.

Land/ocean 판정은 아래 입력을 합성한다.

- Voronoi graph 기반 macro elevation
- continentality
- continent / ocean basin id
- graph basin id
- distance-to-ocean-basin
- coastness
- sea-level contract
- local lake/sink resolution

`target land ratio`만으로는 대륙성이 보장되지 않는다. land ratio는 preview area 또는 graph patch에서
land/open water 비율을 조율하는 보조 tuning일 뿐이다. 대륙성은 별도의 continent/ocean basin
ownership layer가 먼저 제공해야 한다.

launch 정책은 아래처럼 잡는다.

- continent seed와 ocean basin seed를 낮은 빈도의 super-region 또는 plate-like graph에서 먼저 생성한다.
- 각 Voronoi site는 가까운 continent/ocean basin id, continentality, distance-to-continent-core, distance-to-ocean-basin을 받는다.
- signed macro elevation은 이 ownership field 위에 얹히며, sign 하나만으로 대륙/바다 의미를 결정하지 않는다.
- 작은 양수 land component는 기본적으로 island 또는 archipelago candidate다.
- launch 기본값에서도 큰 대륙만 만들지 않고, ocean basin 안에 크고 작은 섬이 일정 비율로 나타날 수 있게 island field를 둔다.
- 섬은 대륙 ownership을 뒤집는 예외가 아니라 별도 deterministic island bump가 continentality를 양수로 끌어올린 결과여야 한다.
- 이후 hydrology/surface 단계에서는 큰 대륙에 붙지 않은 land component를 island로 취급하고, 최소 크기, 해안 폭, 담수 생성 가능성, 식생 밀도 정책을 다르게 줄 수 있어야 한다.

무한 월드에서는 전체 land cell 수와 ocean cell 수를 전역으로 세어 제약할 수 없다. 대신
deterministic super-region ownership, 충분한 padding, component pruning/assimilation 규칙으로
요청 영역마다 같은 대륙성이 재현되게 만든다.

현재 launch 구현은 coarse super-cell 위에 낮은 빈도 continental field, domain warp, island field를
합성해 continent core와 ocean basin center를 만든다. checkerboard처럼 land/ocean을 번갈아 배치하지
않고, 대륙 가장자리가 여러 방향으로 뻗거나 들어가며 ocean basin 안에 크고 작은 섬 후보가 생길 수
있게 한다. site/corner는 가까운 continent core와 ocean basin center까지의 거리, stage 2 base
continentality/elevation seed, island bump를 합성해 ownership과 signed macro elevation을 얻는다.
이 구현은 global target ratio를 세지 않으며, 같은 world-space position은 어떤 padded patch에서
샘플해도 같은 macro annotation을 받는다.

---

## Macro Elevation

macro elevation은 대륙, 바다, 산맥, 능선, 분수계, 강 후보망을 이미 알고 있는 graph-derived
field다. Perlin noise는 이 macro structure를 뒤집는 source가 아니라, 마지막 표면에 국소적인
높낮이와 질감을 더하는 micro relief다.

macro elevation 생성 순서:

1. continent/ocean basin ownership을 정한다.
2. 대륙 내부의 broad elevation gradient를 만든다.
3. coast distance, continent core, basinness를 합성한다.
4. signed macro elevation을 만들되, ocean basin ownership과 sea level contract를 함께 저장한다.
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
- continental core와 coast distance를 이용해 broad mountainness field 생성
- ruggedness와 elevation bias로 ridge 주변 local relief 강화

현재 launch 구현의 ridge 후보는 두 land site의 평균 macro elevation, mountainness/ridgeness,
coast distance를 함께 본다. 즉, 낮은 평지의 random line보다 높은 고도와 산맥성이 겹치는 edge가
더 쉽게 ridge candidate가 된다. 이 단계의 ridge는 최종 능선 mesh가 아니라 hydrology와
Voronoi-derived macro field가 읽을 skeleton이다.

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
2. 대륙성은 target land ratio가 아니라 continent/ocean basin ownership과 connected component 정책으로 보장한다.
3. macro elevation은 Perlin micro relief보다 먼저 계산되어야 한다.
4. mountain/ridge/fault/coast guide는 hydrology보다 먼저 결정되어야 한다.
5. river guide는 routing 결과가 아니라 hydrology가 읽을 후보 annotation이다.
6. coast guide는 connected ocean basin과 land ownership의 경계를 우선한다.
7. graph-derived mountain/ridge/coast guide는 broad field로 확산되어야 하며 raw segment가 그대로 보이면 안 된다.
8. ocean, lake, wetland, coast의 의미 구분은 surface policy와 preview에서 유지되어야 한다.

---

## 현재 구현 상태

- `src/world/generation/macro_map/mod.rs`가 `pub mod macro_map`으로 연결되어 있다.
- `generate_macro_map`은 rayon으로 site/corner/edge annotation을 병렬 생성하고, id 정렬로 deterministic order를 유지한다.
- continent/ocean ownership은 target ratio가 아니라 deterministic super-cell core/basin field와 base graph field 합성으로 정한다.
- signed macro elevation은 land 양수, ocean 음수 contract를 유지한다.
- site/corner annotation은 coastness, distance-ish coast value, mountainness, ridgeness, basinness를 포함한다.
- edge guide는 coast, ridge candidate, fault candidate, hydrology 전 river candidate를 포함한다.
