# heightfield

## 역할

`heightfield`는 Voronoi-derived macro map과 Perlin micro relief를 합성해 final column height를
만드는 계약을 소유한다.

macro elevation은 graph 기반으로 먼저 생성되고, Perlin은 그 위에 얹히는 국소 표면 디테일이다.

---

## 책임

- macro elevation, continent/ocean gradient, ridge/fault field, hydrology valley field 합성
- Voronoi-derived macro noise/gradient map 입력 계약
- Perlin micro relief amplitude와 mask 정책 정의
- river, lake, wetland, floodplain, coast flatten/terrace 반영
- local depression cleanup 또는 clamp 정책 정의

---

## 비책임

- graph topology 생성
- hydrology routing 자체
- final biome/material policy 선택
- voxel fill

---

## Height Formula

최종 높이 후보:

```text
height =
    voronoi_macro_elevation
  + continent_ocean_gradient
  + edge_mountain_ridge_field
  + edge_fault_plateau_field
  - edge_hydrology_valley_field
  - lake_basin_flatten_field
  + noisy_boundary_displacement_field
  + perlin_micro_relief
```

---

## 생성 순서

1. continent/ocean basin과 대륙 내부 macro elevation을 Voronoi graph에서 만든다.
2. Voronoi edge 기반 mountain/ridge/fault/plateau 구조를 먼저 정한다.
3. 이 edge structure와 macro elevation을 바탕으로 edge 기반 hydrology를 설정한다.
4. river, coast, biome boundary, cliff/fault boundary를 noisy boundary로 흔든다.
5. 이 정보를 바탕으로 Voronoi-derived macro noise/gradient map을 만든다.
6. 마지막에 Perlin noise를 합성해 국소 micro elevation을 만든다.

---

## Perlin Micro Relief

Perlin noise 사용 규칙:

- Perlin은 지형의 큰 구조를 발명하지 않는다.
- Perlin amplitude는 ruggedness, slope, hydrology role, coast/lake mask로 제한한다.
- octave별 seed/offset/rotation을 분리해 correlation artifact를 줄인다.
- ocean, lake, river, wetland, floodplain 영역에서는 Perlin을 감쇠하거나 flatten한다.
- mountain/ridge 주변에서는 Perlin이 능선 방향을 보조할 수 있지만, ridge ownership을 뒤집으면 안 된다.

중요한 위험:

- macro elevation과 hydrology가 Perlin보다 먼저 정해지므로 순수 noise-first 방식보다 local minima 문제가 줄어든다.
- 그래도 micro relief 때문에 국소적인 depression은 생길 수 있다.
- river/lake 주변에서는 micro relief clamp, local sink cleanup, lake creation, outlet carve 중 하나 이상의 명시적 처리가 필요하다.

---

## 불변식

1. macro elevation은 Voronoi graph 기반으로 먼저 생성되어야 한다.
2. Perlin micro relief는 macro structure를 뒤집으면 안 된다.
3. hydrology는 final heightfield와 voxel fill 전에 valley/lake/coast 제약으로 반영되어야 한다.
4. Perlin micro relief가 만든 국소 depression은 river/lake/coast policy와 충돌하지 않도록 clamp 또는 cleanup되어야 한다.
5. graph region 사각 경계가 heightfield에 보이면 회귀다.
