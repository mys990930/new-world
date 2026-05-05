# macro_field

## 역할

`macro_field`는 graph-first generator의 8단계인 Voronoi-derived macro field cache를 소유한다.

이 모듈은 새 지형 의미를 다시 noise로 생성하지 않는다. source of truth는 앞 단계의
`VoronoiGraphPatch`, `GraphMacroMap`, `GraphHydrologyGraph`, `BoundaryCache`이며, `macro_field`는
그 vector/graph 결과를 heightfield, preview, chunk column sampler가 빠르게 읽을 수 있는 raster/scalar
tile로 굽는다.

즉 이름은 `noise map`이 아니라 graph-derived field cache다. Perlin/fBM micro relief는 이후 단계에서
이 field 위에 더해지는 표면 디테일이다.

---

## 책임

- tile bounds, resolution, world-space sample spacing 계약 정의
- macro elevation, water/coast/lake/dry basin mask, ridge influence, river valley field channel 정의
- noisy boundary 이후의 canonical curve geometry를 읽어 ridge/coast/river distance envelope를 계산하되,
  chunk/preview sample마다 모든 curve 후보를 반복 탐색하지 않도록 tile 단위 influence pass를 먼저 만든다.
- combined macro height를 만들어 heightfield 합성 전의 큰 지형 형태를 제공
- chunk fill hot path가 graph/macro/hydrology/boundary를 직접 재탐색하지 않도록 중간 cache surface 제공
- stage preview binary가 각 channel과 combined height를 2D topdown map으로 뽑을 수 있는 데이터 제공

---

## 비책임

- Voronoi graph 생성
- macro ownership/elevation resolve
- hydrology routing 또는 selected river 결정
- noisy boundary curve 생성
- Perlin micro relief 생성
- final water surface solve
- biome/material/vegetation/voxel fill

---

## 공개 API

초기 구현은 하나의 tile을 직접 rasterize한다.

```rust
MacroFieldTileConfig::new(origin_x, origin_z, width, height, sample_spacing_blocks)

generate_macro_field_tile(
    &VoronoiGraphPatch,
    &GraphMacroMap,
    &GraphHydrologyGraph,
    &BoundaryCache,
    MacroFieldTileConfig,
) -> MacroFieldTile
```

주요 데이터:

```rust
MacroFieldTileConfig {
    origin,
    width,
    height,
    sample_spacing_blocks,
    ridge_radius_blocks,
    river_radius_blocks,
    coast_radius_blocks,
    ridge_height_scale,
    river_carve_scale,
    coast_flatten_strength,
    lake_flatten_strength,
    boundary_blend_radius_blocks,
}

MacroFieldSample {
    position,
    nearest_site,
    surface_kind,
    macro_elevation,
    ocean_mask,
    coast_mask,
    lake_mask,
    dry_basin_mask,
    ridge_influence,
    river_valley_strength,
    river_distance_blocks,
    river_flow_hint,
    combined_macro_height,
}

MacroFieldTile {
    config,
    samples,
    stats,
}
```

---

## 처리 순서

tile 생성은 먼저 빈 sample grid와 feature influence raster를 만든 뒤, 각 world-space sample point를
병렬로 채운다.

1. tile origin, width, height, sample spacing으로 world-space `(x, z)`를 계산한다.
2. ridge, coast, selected river curve를 tile-local source pixel로 rasterize한다.
   - 이 pass는 curve별 source pixel을 먼저 찍고, chamfer distance propagation으로 `distance to
     nearest ridge/coast/river curve`를 tile channel로 만든다.
   - selected river는 display `flow_accumulation`을 source pixel에 함께 기록하고, distance propagation
     중 가장 가까운 source의 flow hint를 전파한다.
   - 이 구조의 목표는 기존 `O(samples * candidate curves * curve segments)` distance query를
     `O(curve source rasterization + samples)` 계열의 bounded tile pass로 바꾸는 것이다.
   - 현재 launch 구현은 ridge/coast/river influence를 이 raster pass로 처리한다.
3. 먼저 nearest macro site를 찾되, sample point가 canonical noisy boundary curve의 blend radius 안에
   있으면 해당 curve의 양쪽 site를 읽어 noisy curve 기준 owner를 다시 고른다.
   - 이 단계의 visible ownership/mask boundary는 straight nearest-site 선이 아니라 stage 7
     `BoundaryCache`의 `NoisyBoundaryCurve`를 따라야 한다.
   - macro elevation은 primary owner의 값을 기준으로 하되 boundary blend band 안에서는 반대편 site
     elevation을 일부 섞어 계단형 단절을 줄인다.
   - ownership/mask boundary는 정확도 유지를 위해 아직 per-sample noisy-boundary side query를 사용한다.
     후속 최적화는 이 side/blend 판정도 tile edge-classification field로 굽는 것이다.
4. noisy-boundary owner의 `surface_kind`에서 ocean/coast/lake/dry basin mask를 만든다.
5. coast guide edge의 rasterized distance field로 coast mask를 보강한다.
6. ridge guide edge의 rasterized distance field로 `ridge_influence`를 만든다.
   - ridge 자체는 Voronoi edge 위의 산맥 maxima guide다.
   - `ridge_influence`는 그 중심선 주변을 폭 있는 산맥 envelope로 끌어올리기 위한 거리 기반 scalar field다.
7. hydrology selected river segment의 edge id가 가리키는 canonical noisy curve distance와 selected/display flow로 river valley field를 만든다.
   - river 전용 noisy curve는 만들지 않는다.
   - lake boundary/internal/adjacent edge는 hydrology stage에서 selected river가 이미 금지한다.
8. 아래 계열로 combined macro height를 계산한다. 이 단계의 river carve는 최종 물/복셀 carve가
   아니라 heightfield가 읽을 2D valley/carve guide이며, preview에서 보여야 한다.

```text
combined_macro_height =
    macro_elevation
  + ridge_influence * ridge_height_scale
  - river_valley_strength * river_carve_scale
  - coast_flatten
  - lake_flatten
```

`combined_macro_height`는 최종 terrain height가 아니다. 이후 meso feature, Perlin micro relief,
heightfield/water surface composition이 이 값을 읽는다.

---

## Runtime Cache

`macro_field`는 chunk fill hot path를 가볍게 만들기 위한 cache layer다.

```text
graph region cache
-> macro map cache
-> hydrology cache
-> boundary cache
-> macro field tile cache
-> heightfield / water surface cache
-> chunk generation samples column/window data
-> voxel fill writes ChunkData
```

chunk fill은 매 column마다 가장 가까운 ridge curve, river curve, coast curve를 직접 다시 찾지 않아야
한다. worker/cache miss에서 `MacroFieldTile`을 준비하고, chunk generation은 필요한 column/window만
sample한다.

launch 구현은 ridge/coast/river influence를 per-sample polyline query 대신 tile-local raster pass로
굽는다. 이 pass는 canonical noisy curve를 source pixel로 찍고 distance propagation으로 envelope를
만든다. ownership과 macro elevation의 noisy-boundary side/blend 판정은 아직 per-sample query로 남아
있는데, 이것은 visible mask boundary 정확도를 지키기 위한 보수적 선택이다. 4K preview의 남은 주된
비용은 이 ownership side query와 nearest site lookup이며, 후속 최적화는 owner classification field를
같은 tile cache에 굽는 것이다.

---

## Preview

프로젝트 요구상 각 generation stage는 topdown preview binary로 확인 가능해야 한다.

`macro_field` preview는 최소한 아래 channel을 각각 2D로 출력할 수 있어야 한다.

- macro elevation
- ocean/coast/lake/dry basin mask. coast/lake/ocean 경계는 noisy boundary를 따라 보여야 한다.
- ridge influence
- river valley strength/distance/flow hint. selected hydrology edge path의 canonical noisy curve 주변
  carve guide가 보여야 하며, 이 guide는 tile influence raster pass 결과를 사용한다.
- combined macro height. river valley carve와 ridge raise가 Perlin 전 높이에 반영되어야 한다.

중간 단계 preview는 2D gradient map이면 충분하다. 이후 heightfield stage의 최종 산출물은 white
texture 기반 top-down heightfield render와 simple lighting으로 검증한다.

---

## 불변식

1. `macro_field`는 graph/macro/hydrology/boundary를 대체하는 source of truth가 아니다.
2. Perlin micro relief는 `macro_field` 이후에 합성되며 macro ownership을 뒤집으면 안 된다.
3. ridge guide는 edge maxima skeleton이고, ridge influence는 heightfield가 읽는 주변 envelope다.
4. river valley는 hydrology selected segment만 읽어야 하며, macro river candidate를 강으로 해석하면 안 된다.
5. river geometry는 selected edge id의 canonical noisy boundary curve를 따른다.
6. tile sample fill은 deterministic해야 하며, 병렬 scheduling이 sample 순서나 값에 영향을 주면 안 된다.
7. combined macro height는 finite 값이어야 하고 preview 가능한 범위를 유지해야 한다.

---

## 현재 구현 상태

- `src/world/generation/macro_field/mod.rs`가 `MacroFieldTileConfig`, `MacroFieldSample`,
  `MacroFieldTile`, `generate_macro_field_tile`을 제공한다.
- sample fill은 rayon parallel iterator를 사용하고, index 기반 위치 계산으로 deterministic order를 유지한다.
- launch rasterizer는 nearest macro site를 기본 lookup으로 사용하되, boundary blend radius 안에서는
  canonical noisy boundary curve의 side test로 owner/mask/elevation boundary를 고른다.
- ridge/coast/river influence는 selected edge id가 참조하는 canonical noisy curve를 tile source pixel로
  rasterize한 뒤 chamfer distance field로 만든다. 이로써 sample마다 curve 후보와 polyline segment를
  반복 탐색하던 비용을 줄인다.
- signed polygon containment와 더 정교한 multi-edge blend는 후속 단계에서 확장할 수 있지만,
  visible macro field boundary가 straight nearest-site raster로 되돌아가면 회귀다.
