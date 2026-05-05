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
- noisy boundary 이후의 canonical curve geometry를 읽어 ridge/coast/river distance envelope를 계산
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

tile 생성은 먼저 빈 sample grid를 만든 뒤, 각 world-space sample point를 병렬로 채운다.

1. tile origin, width, height, sample spacing으로 world-space `(x, z)`를 계산한다.
2. nearest macro site를 찾아 `surface_kind`와 `signed_macro_elevation`을 읽는다.
   - launch 구현은 polygon containment 대신 deterministic nearest-site rasterization으로 시작한다.
   - 계약상 이 단계는 noisy boundary 이후에 실행되며, 이후 구현은 noisy boundary 기반 containment/blend로 대체될 수 있다.
3. `surface_kind`에서 ocean/coast/lake/dry basin mask를 만든다.
4. coast guide edge의 canonical noisy curve distance로 coast mask를 보강한다.
5. ridge guide edge의 canonical noisy curve distance로 `ridge_influence`를 만든다.
   - ridge 자체는 Voronoi edge 위의 산맥 maxima guide다.
   - `ridge_influence`는 그 중심선 주변을 폭 있는 산맥 envelope로 끌어올리기 위한 거리 기반 scalar field다.
6. hydrology selected river segment의 edge id가 가리키는 canonical noisy curve distance와 selected/display flow로 river valley field를 만든다.
   - river 전용 noisy curve는 만들지 않는다.
   - lake boundary/internal/adjacent edge는 hydrology stage에서 selected river가 이미 금지한다.
7. 아래 계열로 combined macro height를 계산한다.

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

---

## Preview

프로젝트 요구상 각 generation stage는 topdown preview binary로 확인 가능해야 한다.

`macro_field` preview는 최소한 아래 channel을 각각 2D로 출력할 수 있어야 한다.

- macro elevation
- ocean/coast/lake/dry basin mask
- ridge influence
- river valley strength/distance/flow hint
- combined macro height

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
- launch rasterizer는 nearest macro site와 noisy curve distance envelope를 사용한다.
- signed distance/polygon containment, high quality boundary blend, macro field preview binary는 후속 worker 또는 다음 단계에서 확장해야 한다.
