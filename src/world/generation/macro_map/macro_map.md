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
- edge 기반 mountain/ridge/fault/plateau/coast guide 선택
- ridge/fault/coast guide를 broad field로 확산
- hydrology가 읽을 drainage divide, basin, outlet 후보 제공

---

## 비책임

- river routing 확정
- noisy boundary curve 생성
- Perlin micro relief 합성
- final surface material 선택
- voxel fill

---

## Continent, Ocean, Lake, Coast

water는 단순히 `height < sea_level`로 끝내면 안 된다. world는 물의 의미를 구분해야 한다.

이 프로젝트는 Amit의 작은 island map과 달리 큰 대륙과 큰 바다가 공존하는 구조를 목표로 한다.
대륙은 하나의 작은 섬이 아니라 장거리 macro ownership을 가진 land mass이며, 그 내부에 산맥,
분수계, 강, 호수, 습지, 평야, 해안 지형이 배치된다.

- ocean: 큰 바다 또는 외부 ocean basin과 연결된 물
- continent: 큰 land mass와 그 내부 macro elevation / drainage ownership
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

---

## Macro Elevation

macro elevation은 대륙, 바다, 산맥, 능선, 분수계, 강 후보망을 이미 알고 있는 graph-derived
field다. Perlin noise는 이 macro structure를 뒤집는 source가 아니라, 마지막 표면에 국소적인
높낮이와 질감을 더하는 micro relief다.

macro elevation 생성 순서:

1. continent/ocean basin ownership을 정한다.
2. 대륙 내부의 broad elevation gradient를 만든다.
3. coast distance, continent core, basinness를 합성한다.
4. edge 기반 mountain/ridge/fault/plateau 후보를 먼저 정한다.
5. ridge/fault/coast skeleton을 broad field로 확산한다.
6. hydrology가 사용할 divide, basin, outlet 후보를 annotation한다.

---

## Mountain And Ridge Structure

산맥은 단순히 높은 noise가 아니다.

world는 graph 위에 mountain belt / ridge chain / fault line 후보를 소유해야 한다. 구현 방식은
아래 중 하나 또는 조합이 될 수 있다.

- graph site chain을 mountain belt로 선택
- edge chain을 fault/ridge candidate로 선택
- plate-like region boundary를 uplift source로 사용
- continental core와 coast distance를 이용해 broad mountainness field 생성
- ruggedness와 elevation bias로 ridge 주변 local relief 강화

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
2. macro elevation은 Perlin micro relief보다 먼저 계산되어야 한다.
3. mountain/ridge/fault/coast guide는 hydrology보다 먼저 결정되어야 한다.
4. graph-derived mountain/ridge/coast guide는 broad field로 확산되어야 하며 raw segment가 그대로 보이면 안 된다.
5. ocean, lake, wetland, coast의 의미 구분은 surface policy와 preview에서 유지되어야 한다.
