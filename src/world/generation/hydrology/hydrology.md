# hydrology

## 역할

`hydrology`는 Voronoi corner와 selected edge를 기반으로 한 graph-first 물 흐름 계약을 소유한다.

polygon 경계는 강 후보망이지만, 모든 경계가 강이 되어서는 안 된다. hydrology는 macro elevation,
ridge, coast, basin 정보를 읽어 downhill routing, watershed, river segment, lake/outlet 처리를
계산하고, 이후 heightfield가 valley와 water surface를 알 수 있게 제약을 제공한다.

---

## 책임

- watershed, drainage node, river segment 표현
- selected graph edge를 divide, headwater, tributary, trunk, floodplain, outlet으로 분류
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

## 처리 순서

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

1. Voronoi edge는 hydrology 후보선이지 자동 river가 아니다.
2. selected river segment는 descending 또는 outlet-carved graph logic을 따라야 한다.
3. local minima는 lake, sink, outlet carve 중 하나로 명시되어야 한다.
4. selected river path는 generation order와 chunk order에 독립적이어야 한다.
5. river, lake, ocean, wetland는 같은 water mask로 뭉개지지 않고 의미가 구분되어야 한다.
6. final river geometry는 raw straight edge가 아니라 spline/domain-warped realization을 사용해야 한다.

---

## 현재 구현 상태

- 현재는 graph data contract scaffold 단계다.
- future work는 continuous field와 macro elevation 이후, heightfield synthesis 이전에 watershed routing을 풀어야 한다.
