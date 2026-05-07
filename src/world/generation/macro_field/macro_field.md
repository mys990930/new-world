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
- combined macro height를 block-space로 해석한 contour 진단 layer 제공
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

MacroFieldContourSet {
    step_blocks,
    major_every,
    min_level_blocks,
    max_level_blocks,
    total_segment_count,
    levels,
}
```

`MacroFieldContourSet`은 terrain source of truth가 아니다. 이 구조는 `MacroFieldTile.samples[].combined_macro_height`
를 heightfield 직전 block-height scale로 변환한 뒤 Marching Squares로 추출한 진단 layer다. contour는
macro field가 heightfield로 넘어가기 직전에 연속적으로 읽히는지 확인하는 preview surface다. 이후
heightfield는 같은 block-height contour level domain을 사용해 column band interpolation을 할 수 있지만,
Marching Squares segment 자체를 terrain source로 직접 소비해서는 안 된다.

Contour block-height scale은 heightfield launch slice와 맞춘다.

```text
combined_macro_height -0.75 .. 1.25
-> -48 .. 160 blocks
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
   - 일반 boundary blend는 coast mask가 아니다. coast mask는 explicit ocean-owned surface와
     non-ocean surface 사이의 coast guide 또는 ocean-coast site context에서만 올라가야 한다.
   - dry basin, lake, wetland와 주변 land 사이의 noisy boundary는 ownership/macro elevation blend에는
     참여할 수 있지만 shoreline/coast flatten으로 처리하면 안 된다.
5. coast guide edge의 rasterized distance field로 coast mask를 보강한다. 이 source는 `macro_map`의
   explicit coast guide만 사용하며, 모든 Voronoi boundary edge를 coast처럼 splat하면 회귀다.
6. ridge guide edge의 rasterized distance field로 `ridge_influence`를 만든다.
   - ridge 자체는 Voronoi edge 위의 산맥 maxima guide다.
   - `ridge_influence`는 그 중심선 주변을 폭 있는 산맥 envelope로 끌어올리기 위한 거리 기반 scalar field다.
   - 이 단계는 아직 Perlin micro relief 전이므로 낮은 꼬리값이 tile 전체에 grain처럼 깔리면 안 된다.
     launch 구현은 ridge distance envelope의 낮은 값은 잘라내고, active fraction을 stats/preview에 기록한다.
   - 현재 launch slice에서는 `ridge_influence`를 combined height에 더하지 않는다. ridge channel은
     guide/source 진단용으로 유지하지만, ridge raise는 broad mountain elevation model이 들어올 때까지
     disabled/stub 상태다. 기존 narrow ridge envelope가 1블록 등고선 기준에서 pinpoint maxima를 만들어
     contour가 층마다 불연속적으로 튀어 보였기 때문이다.
7. hydrology selected river segment의 edge id가 가리키는 canonical noisy curve distance와 selected/display flow로 river valley field를 만든다.
   - river 전용 noisy curve는 만들지 않는다.
   - lake boundary/internal/adjacent edge는 hydrology stage에서 selected river가 이미 금지한다.
   - river valley width와 depth는 모두 selected/display flow에서 파생한다. launch 기본 정책은
     `flow_hint = clamp(sqrt(flow_accumulation) / 32, 0, 1)`을 만들고,
     `width = 20 + (144 - 20) * flow_hint^1.35` blocks 범위를 사용한다. 중심부 carve depth도
     `0.18..1.0` 범위에서 `flow_hint^1.15`로 커진다. 따라서 상류는 좁고 얕게 빠르게 사라지고,
     하류 trunk에서만 넓고 깊은 valley guide가 보여야 한다.
8. 아래 계열로 combined macro height를 계산한다. 이 단계의 river carve는 최종 물/복셀 carve가
   아니라 heightfield가 읽을 2D valley/carve guide이며, preview에서 보여야 한다.

```text
combined_macro_height =
    macro_elevation
  - river_valley_strength * river_carve_scale
  - coast_flatten
  - lake_flatten
```

`ridge_height_scale`의 launch 기본값은 `0`이다. 즉 현재 `combined_macro_height`는 ridge guide를
높이 maxima로 직접 더하지 않고, macro elevation과 river/coast/lake/dry-basin 제약만 합성한다.
ridge guide는 여전히 별도 channel과 stats로 확인할 수 있으며, 이후 stage에서 연결된 broad mountain
elevation model을 설계한 뒤 재도입한다.

`combined_macro_height`는 최종 terrain height가 아니다. 이후 meso feature, Perlin micro relief,
heightfield/water surface composition이 이 값을 읽는다.

Dry basin은 lake/ocean처럼 water flatten 대상이 아니다. `DryBasin` mask는 폐쇄 저지대라는
surface/context를 드러내지만, combined height에서는 얕은 above-sea-level land floor로 clamp한다.
주변 rim이나 사면은 이후 heightfield/water solve에서 더 정교하게 만들 수 있지만, macro field
단계에서 dry basin 주변을 물처럼 낮추거나 분지 바깥이 분지 floor보다 낮아 보이게 만드는 것은 회귀다.

9. 필요한 경우 `combined_macro_height`에서 contour 진단 layer를 추출한다.
   - contour 추출은 Marching Squares 기반이다.
   - level은 normalized scalar가 아니라 heightfield 직전 block-height 기준이다.
   - 기본 preview step은 8 blocks, major contour는 5 level마다 40 blocks 간격이다.
   - preview contour 색은 height에 따라 달라져야 한다. 낮은/oceanward contour는 푸른 계열,
     높은 contour는 붉은/주황 계열을 사용하고, sea level `y = 0` contour는 별도 preview 색상으로
     구분할 수 있어야 한다.
   - flat field는 contour를 만들지 않아야 하며, 모든 segment endpoint는 finite world-space point여야 한다.

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
  dry basin은 별도 mask/color로 표시되며 coast 노란색과 구분되어야 한다.
- ridge influence
- river valley strength/distance/flow hint. selected hydrology edge path의 canonical noisy curve 주변
  carve guide가 보여야 하며, 이 guide는 tile influence raster pass 결과를 사용한다. 상류는 좁고
  얕게, 하류는 넓고 깊게 보여야 한다.
- combined macro height. river valley carve가 Perlin 전 높이에 반영되어야 하며,
  preview 색상은 진단용 heat map이 아니라 muted blue-gray, green-gray, olive/gray, pale gray로 이어지는
  subtle terrain ramp를 사용해 pre-Perlin topdown 지형 표면처럼 읽혀야 한다.
- contour. heightfield 직전 block-height scale의 combined macro height 등고선을 보여준다. minor
  contour, major contour, sea-level contour는 서로 구분되어야 하며, contour line 색상은 낮은 곳의
  푸른 계열에서 높은 곳의 붉은 계열로 이어져야 한다. contour channel에서는 noisy Voronoi edge
  overlay가 contour 판독을 방해하지 않아야 한다.

preview metadata/stdout은 ridge active sample fraction, dry basin sample count와 dry basin combined
height range를 기록한다. macro field stage에는 아직 micro Perlin이 없으므로 lit preview의 촘촘한
grain은 ridge/coast/river/boundary blend 또는 lighting contrast에서 온 것이다. ordinary cell
interior가 micro detail처럼 보이면 ridge tail/lighting/contrast를 먼저 의심해야 한다.

`macro`, `combined`, `lit` preview는 tile이나 image마다 local min/max를 잡아 색을 다시 늘리지
않는다. height context가 이어져 보이도록 launch 기준 absolute normalized scale을 사용한다.

```text
macro elevation preview: -1.00 .. 1.00
combined macro height preview: -0.75 .. 1.25
```

per-image min/max와 robust percentile은 metadata/stdout 진단값일 뿐 color scale의 source가 아니다.
모든 channel은 canonical noisy Voronoi graph edge overlay를 기본으로 표시해야 한다. 사용자가
terrain tile 경계를 확인한다고 말할 때의 1차 의미는 macro-field cache grid가 아니라, stage 7의
`BoundaryCache`가 제공하는 noisy edge geometry다. 이 overlay는 field 값을 가리지 않는 faint
reference layer여야 하며, 기본 alpha는 강한 선 레이어가 아니라 위치 확인용 수준이어야 한다.
straight nearest-site 경계가 아니라 canonical noisy curve를 따른다.

macro-field cache tile boundary grid는 보조 진단용으로 얇게 유지할 수 있다. 이 grid가 Voronoi graph
edge보다 강하게 보이거나 height normalization 단위처럼 읽히면 회귀다.

`lit` preview는 combined macro height 데이터를 바꾸지 않고, lighting 계산에만 broader low-pass
height와 central-difference radius를 적용할 수 있다. 목적은 개별 tile/sample 단위 lighting을 보는
것이 아니라, 흰색 재질 위에서 continent-scale 산지, 분지, 해안, 강 계곡의 고저차를 broad
hillshade로 읽는 것이다. sample 단위 단절과 noisy-boundary/mask transition이 조명으로 과장되어
타일마다 오돌토돌하게 융기한 것처럼 보이면 회귀지만, 반대로 broad relief contrast가 거의 사라져
단색 회색처럼 보이는 것도 회귀다. combined/macro channel 값 자체를 무턱대고 blur하면 안 된다.
lit channel의 Voronoi edge overlay는 다른 channel보다 더 희미해야 한다. lit은 graph 위치 확인보다
broad hillshade 판독이 우선이며, edge는 거의 참조선 수준이어야 한다.

중간 단계 preview는 2D gradient map이면 충분하다. 이후 heightfield stage의 최종 산출물은 white
texture 기반 top-down heightfield render와 simple lighting으로 검증한다.

---

## 불변식

1. `macro_field`는 graph/macro/hydrology/boundary를 대체하는 source of truth가 아니다.
2. Perlin micro relief는 `macro_field` 이후에 합성되며 macro ownership을 뒤집으면 안 된다.
3. ridge guide는 edge maxima skeleton이고, ridge influence는 heightfield가 읽을 수 있는 주변 envelope
   진단 channel이다. 현재 launch slice에서는 ridge raise가 combined height에서 disabled 상태다.
   broad mountain elevation model 없이 narrow ridge envelope만 높이에 더하면 1블록 contour 기준에서
   pinpoint maxima와 불연속적인 등고선 밀도 변화를 만들기 때문이다.
   Perlin 전 단계에서 ridge influence가 거의 모든 tile sample에 nonzero low-level grain으로 깔리면
   안 된다. ridge를 높이로 재도입할 때는 guide 위 한 점만 밝은 pinpoint로 남지 않고, selected ridge
   path를 따라 연결된 mountain belt shoulder가 보여야 한다.
4. river valley는 hydrology selected segment만 읽어야 하며, macro river candidate를 강으로 해석하면 안 된다.
5. river geometry는 selected edge id의 canonical noisy boundary curve를 따른다. river valley width와
   carve depth는 selected/display flow에 비례해야 하며, 고정 폭 corridor를 모든 강에 적용하면 안 된다.
6. tile sample fill은 deterministic해야 하며, 병렬 scheduling이 sample 순서나 값에 영향을 주면 안 된다.
7. combined macro height는 finite 값이어야 하고 preview 가능한 범위를 유지해야 한다.
8. dry basin은 water mask가 아니며, combined macro height에서 lake/ocean flatten을 적용하지 않는다.
9. dry basin 또는 lake/wetland와 land 사이의 boundary blend는 coast mask를 만들지 않는다. coast
   mask는 connected ocean basin과 non-ocean terrain 사이의 explicit coast context만 읽는다.
10. preview renderer는 macro field tile 내부를 local low/high로 정규화하지 않고, 문서화된 absolute
   normalized scale을 사용해야 한다.
11. contour segment는 preview/debug layer이며, source graph/macro/hydrology/boundary나 heightfield
    scalar source를 대체하지 않는다. heightfield가 contour-guided mode를 사용할 때도 같은 level/step
    domain을 공유할 뿐, contour polyline을 새 terrain source로 삼지 않는다.

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
