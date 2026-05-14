# macro_field

## 역할

`macro_field`는 graph-first generator의 10단계인 Voronoi-derived macro field cache를 소유한다.

이 모듈은 새 지형 의미를 다시 noise로 생성하지 않는다. source of truth는 앞 단계의
`VoronoiGraphPatch`, `GraphMacroMap`, `RiverPlan`, final cell context,
`BoundaryCache`이며, `macro_field`는
그 vector/graph 결과를 heightfield, preview, chunk column sampler가 빠르게 읽을 수 있는 raster/scalar
tile로 굽는다.

즉 이름은 `noise map`이 아니라 graph-derived field cache다. Perlin/fBM micro relief는 이후 단계에서
이 field 위에 더해지는 표면 디테일이다.

---

## 책임

- tile bounds, resolution, world-space sample spacing 계약 정의
- macro elevation, water/coast/lake/dry basin mask, ridge influence, river valley field, river bed hint, final cell
  context/biome influence channel 정의
- noisy boundary 이후의 canonical curve geometry를 읽어 ridge/coast/river distance envelope를 계산하되,
  chunk/preview sample마다 모든 curve 후보를 반복 탐색하지 않도록 tile 단위 influence pass를 먼저 만든다.
- combined macro height를 만들어 heightfield 합성 전의 큰 지형 형태를 제공
- combined macro height를 block-space로 해석한 contour 진단 layer 제공
- chunk fill hot path가 graph/macro/hydrology/river-plan/final-cell-context/boundary를 직접 재탐색하지 않도록 중간 cache surface 제공
- stage preview binary가 각 channel과 combined height를 2D topdown map으로 뽑을 수 있는 데이터 제공

---

## 비책임

- Voronoi graph 생성
- macro ownership/elevation resolve
- hydrology routing 또는 selected river 결정
- river reach type, broad valley/bed width/depth morphology 결정
- noisy boundary curve 생성
- Perlin micro relief 생성
- final water surface solve
- biome resolve
- material/vegetation/voxel fill

---

## 공개 API

초기 구현은 하나의 tile을 직접 rasterize한다.

```rust
MacroFieldTileConfig::new(origin_x, origin_z, width, height, sample_spacing_blocks)

generate_macro_field_tile(
    &VoronoiGraphPatch,
    &GraphMacroMap,
    &RiverPlan,
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
    lake_flatten_strength,
    boundary_blend_radius_blocks,
    boundary_roughness_blocks,
}

MacroFieldSample {
    position,
    nearest_site,
    surface_kind,
    biome_context,
    biome,
    macro_elevation,
    ocean_mask,
    coast_mask,
    lake_mask,
    dry_basin_mask,
    ridge_influence,
    river_valley_strength,
    river_distance_blocks,
    river_flow_hint,
    river_bed_depth_hint,
    river_bank_roughness_hint,
    river_gravel_hint,
    river_cutbank_hint,
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

Contour block-height scale은 heightfield launch slice와 맞춘다. `combined_macro_height = 0`은
항상 signed sea level인 `0 block`이다.

```text
combined_macro_height -0.50 -> -1024 blocks
combined_macro_height  0.00 ->   0 blocks
combined_macro_height  1.00 -> 2048 blocks
```

현재 실험 관심 구간은 `combined_macro_height -0.25..0.75`이며, 같은 piecewise slope에서
`-512..1536 blocks`로 매핑된다. `-0.5..1.0` 바깥 값은 contour/heightfield block conversion에서
각각 `-1024` 또는 `2048` block으로 포화된다. 이 변환은 preview 렌더링 트릭이 아니라 contour
추출과 heightfield band resolve가 공유하는 실제 block-domain 변환이다.

---

## 처리 순서

tile 생성은 먼저 빈 sample grid와 feature influence raster를 만든 뒤, 각 world-space sample point를
병렬로 채운다.

1. tile origin, width, height, sample spacing으로 world-space `(x, z)`를 계산한다.
2. ridge, coast, river plan guide를 tile-local influence field로 rasterize한다.
   - ridge/coast는 launch 성능을 위해 source pixel과 chamfer distance propagation을 계속 사용할 수 있다.
   - river는 stage 7 `RiverPlan`의 selected edge id와 flow hint를 읽고, 해당 edge의 canonical noisy
     curve를 그대로 tile-local influence field로 굽는다. macro field는 flow-smoothed realization curve,
     별도 thalweg offset, 하구 fan geometry를 만들지 않는다.
   - 각 sample cell은 river valley strength, nearest distance, blended flow hint, bed/roughness/gravel
     diagnostic hint를 보존한다. combined height에는 단순 broad valley lowering만 반영하며, narrow bed
     단면을 macro field에서 직접 완성하지 않는다.
   - 이 구조의 목표는 기존 `O(samples * candidate curves * curve segments)` distance query를
     `O(curve source rasterization + samples)` 계열의 bounded tile pass로 바꾸는 것이다.
   - 현재 launch 구현은 ridge/coast/river influence를 이 raster pass로 처리한다.
   - river anti-aliased bake는 selected river segment의 canonical curve 선분들을 굽고, endpoint를
     공유하는 river source들을 하나의 connected raster component로 묶는다. 같은 component 안에서는
     valley strength를 soft union으로 합성해 bend/joint와 broad stroke overlap에서 pointed cusp나
     원형 blob chain이 생기지 않게 한다. 서로 endpoint를 공유하지 않는 가까운 river component끼리는
     nearest local ownership을 유지해 독립적인 평행 하천이 하나의 넓은 corridor로 합쳐지지 않게 한다.
3. 먼저 nearest macro site를 찾되, sample point가 canonical noisy boundary curve의 blend radius 안에
   있으면 해당 curve의 양쪽 site를 읽어 noisy curve 기준 owner를 다시 고른다.
   - 이 단계의 visible ownership/mask boundary는 straight nearest-site 선이 아니라 stage 9
     `BoundaryCache`의 `NoisyBoundaryCurve`를 따라야 한다.
   - macro elevation scalar의 source는 `macro_map`의 `MacroSite.signed_macro_elevation`이다.
     `macro_field`는 가까운 macro site source elevation을 짧은 범위에서 보간해 continuous scalar field를
     만든다. site 중심에서는 원본 elevation을 그대로 보존해야 하며, 장거리 land-group 평균이나
     terrain-kind-specific profile을 만들면 안 된다. 보간 candidate를 fixed top-K로 자르면 K번째/다음
     site의 higher-order Voronoi boundary가 xz 평면에서 직선 단차로 보일 수 있으므로, compact support
     falloff가 0으로 수렴하는 radius 안의 site 전체를 사용해야 한다.
  - boundary blend band 안에서도 scalar height와 owner/mask 판정을 분리한다. owner/mask 판정은 여전히
    가장 가까운 noisy boundary side query를 따르지만, scalar height는 선택된 owner site의 hard value가
    아니라 같은 local interpolation field를 읽는다. explicit coast boundary는 coast mask/influence
    source로 남지만, macro_field 단계에서 coast-specific elevation profile을 별도로 적용하지 않는다.
    coast/land owner sample의 급경사가 보이면 macro_map source elevation 또는 이후 stage로 진단해야 하며,
    macro_field가 숨기거나 보정하지 않는다.
   - ownership/mask boundary는 정확도 유지를 위해 아직 per-sample noisy-boundary side query를 사용한다.
     후속 최적화는 이 side/blend 판정도 tile edge-classification field로 굽는 것이다.
4. noisy-boundary owner의 `surface_kind`에서 ocean/coast/lake/dry basin mask를 만든다.
   - 일반 boundary blend는 coast mask가 아니다. coast mask는 explicit ocean-owned surface와
     non-ocean surface 사이의 coast guide 또는 ocean-coast site context에서만 올라가야 한다.
   - dry basin, lake, wetland와 주변 land 사이의 noisy boundary는 owner/mask 판정에는 참여할 수 있지만
     macro elevation source를 섞거나 coast mask를 만들면 안 된다.
   - lake/wetland와 주변 non-lake 사이의 lowering factor는 hard owner mask를 그대로 쓰지 않고,
     canonical noisy boundary까지의 거리로 양쪽에서 연속적으로 전이한다. lake mask 자체는 진단/정책용
     ownership channel로 유지하지만, combined height lowering은 이 smooth lake factor를 사용한다.
     lake lowering은 단일 절대 target으로 완전히 flatten하지 않고, source macro elevation에서 거리 기반
     U자형 bed carve를 적용한다. lake/non-lake boundary 자체에서는 carve를 0으로 시작하고 lake 내부로
     들어갈수록 깊어져야 하며, land side를 함께 깎아 테두리 cusp나 주변 land 단차를 만들면 안 된다.
   - noisy boundary owner resolve가 raster sample grid에서 disconnected ocean-owned 파편을 만들 수
     있다. macro_field는 tile edge에 닿은 ocean component와 `OceanBasin` source를 포함한 큰 detached
     ocean component는 보존하되, tile 안쪽의 `CoastOcean`-only component는 크기와 무관하게 water mask로
     확정하지 않고 land/coast sample로 되돌린다. fragment 한계는 sample 수가 아니라 block 면적으로
     해석한다. 이 후처리는 source graph나 lake/wetland mask를 바꾸지 않는 raster cache 정합성 guard다.
5. coast guide edge의 rasterized distance field로 coast mask를 보강한다. 이 source는 `macro_map`의
   explicit coast guide만 사용하며, 모든 Voronoi boundary edge를 coast처럼 splat하면 회귀다.
   coast distance는 source guide를 바꾸지 않는 world-space roughness offset을 거쳐 mask로
   변환한다. 이 roughness는 coast ownership이나 ocean/lake 판정을 뒤집지 않고, 지나치게 매끈한
   visible shoreline band를 덜 인조적으로 보이게 하는 raster 표현 계층이다. launch 기본값은
   96 blocks이며, 한 블록이 0.5m이므로 약 48m 규모의 visible shoreline variation을 허용한다.
6. ridge guide edge의 rasterized distance field로 `ridge_influence`를 만든다.
   - ridge 자체는 Voronoi edge 위의 산맥 maxima guide다.
   - `ridge_influence`는 그 중심선 주변을 폭 있는 산맥 envelope로 끌어올리기 위한 거리 기반 scalar field다.
   - 이 단계는 아직 Perlin micro relief 전이므로 낮은 꼬리값이 tile 전체에 grain처럼 깔리면 안 된다.
     launch 구현은 ridge distance envelope의 낮은 값은 잘라내고, active fraction을 stats/preview에 기록한다.
   - 현재 launch slice에서는 `ridge_influence`를 combined height에 더하지 않는다. ridge channel은
     guide/source 진단용으로 유지하지만, ridge raise는 broad mountain elevation model이 들어올 때까지
     disabled/stub 상태다. 기존 narrow ridge envelope가 1블록 등고선 기준에서 pinpoint maxima를 만들어
     contour가 층마다 불연속적으로 튀어 보였기 때문이다.
7. river plan의 selected edge와 flow hint를 읽어 river valley field와 diagnostic bed hint를 만든다.
   - river 전용 noisy curve나 새 river topology를 만들지 않는다.
   - lake boundary/internal/adjacent edge는 hydrology stage에서 selected river가 이미 금지한다.
   - macro_field가 combined height에 반영하는 값은 단순 river valley strength다. 좁은 river bed,
     U/V 단면, cutbank/gravel 편향, 하구 fan은 이 단계에서 만들지 않는다.
   - `river_valley_strength`는 downstream water/river mask가 읽는 0..1 공간 profile이다. 깊이 정보는
     현재 단순 diagnostic hint로만 전달하며, 현실적인 단면 carve는 heightfield/water/surface 단계에서
     다시 설계한다.
8. final cell context를 sample 위치에 맞춰 raster/cache한다.
   - final temperature, final hydration, hydrology role, water proximity, rain shadow, biome influence는
     stage 8에서 이미 resolve된 값이다.
   - macro_field는 biome을 새로 분류하지 않는다. 필요한 경우 boundary/domain-warped blend와 sample
     interpolation을 적용해 downstream stage가 읽을 cache channel로 옮긴다.
   - biome influence가 hard owner straight boundary처럼 보이면 회귀다. visible material 경계는
     surface_plan에서 hydrology role, slope/exposure, dithering과 함께 최종 표현된다.
9. 아래 계열로 combined macro height를 계산한다. 이 단계의 river contribution은 최종 물/복셀
   channel carve가 아니라 heightfield가 읽을 broad valley guide이며, preview에서 보여야 한다.

```text
combined_macro_height =
    ocean_bathymetry_shape(macro_elevation + ridge_raise)        for ocean-owned samples
    macro_elevation + ridge_raise - broad_river_valley_carve      for ordinary non-lake samples
    macro_elevation + ridge_raise - broad_river_valley_carve
      - lake_u_bed_carve                                         for lake/wetland samples
```

`ridge_height_scale`의 launch 기본값은 `0`이다. 즉 현재 `combined_macro_height`는 ridge guide를
높이 maxima로 직접 더하지 않고, macro elevation과 ocean bathymetry/rivers/lakes 제약만 합성한다.
ridge guide는 여전히 별도 channel과 stats로 확인할 수 있으며, 이후 stage에서 연결된 broad mountain
elevation model을 설계한 뒤 재도입한다.

`combined_macro_height`는 최종 terrain height가 아니다. 이후 meso feature, Perlin micro relief,
heightfield/water surface composition이 이 값을 읽는다.

Ocean은 lake flatten을 공유하지 않는다. `OceanBasin`/`CoastOcean` sample은 macro_map에서 넘어온
음수 `macro_elevation`을 continental shelf -> continental slope -> ocean basin처럼 읽히는 S-curve
bathymetry로 변환한다. 해수면에 매우 가까운 값은 고정 shallow plane으로 점프하지 않고 source depth에
가깝게 유지되어 coast->sea y continuity를 보존한다. continental shelf는 좁게 유지하고, 그 뒤의 중간
음수 구간은 slope처럼 빠르게 깊어지며, 큰 음수 구간은 basin depth를 유지해야 한다. ocean combined
height가 단일 얕은 값으로 눌리면 macro_map의 deep/shallow ocean 신호가 사라지므로 회귀다.

Dry basin은 lake/ocean처럼 water flatten 대상이 아니다. `DryBasin` mask는 폐쇄 저지대라는
surface/context와 통계만 드러낸다. dry basin으로 분류되었다면 낮은 분지 맥락은 앞 단계의 macro
elevation이 이미 들고 있어야 하므로, macro field는 별도 dry-basin floor lowering, rim raise, 또는
water flatten을 추가하지 않는다. macro field 단계에서 dry basin 주변을 물처럼 낮추거나, 별도 profile로
분지 내부를 다시 조각하면 회귀다.

macro_map이 1~3 site/cell tiny local-minima lake로 승격한 작은 `LakeCandidate`는 dry basin bowl이
아니라 lake/wetland mask 계열로 rasterize한다. 이 작은 호수는 river-side일 필요가 없으며, selected
river가 연결되지 않아도 lake footprint는 유지한다. lake bed는 `macro_elevation` source의 local relief를
일부 보존한 채 boundary에서 중심으로 갈수록 더 낮아지는 U자형 carve로 표현한다. 단일 normalized height로
전체 lake를 눌러 완전 평면 바닥을 만들면 회귀다. 다만 hydrology가 selected flow를 연결한 경우에는
기존 lake boundary/internal/adjacent edge 금지와 inlet/outlet marker 계약을 그대로 따라야 한다.

Coast mask는 진단과 downstream surface/water policy를 위한 channel이며 `combined_macro_height`를
낮추거나 blend하지 않는다. explicit coast boundary는 coast mask/influence source로 남지만, macro_field는
coast-specific boundary profile을 적용하지 않는다. 해안 바로 안쪽 contour가 어떻게 변하는지는
`macro_map` source elevation으로 진단한다.

10. 필요한 경우 `combined_macro_height`에서 contour 진단 layer를 추출한다.
   - contour 추출은 Marching Squares 기반이다.
   - level은 normalized scalar가 아니라 heightfield 직전 block-height 기준이다.
   - 기본 preview step은 32 blocks, major contour는 5 level마다 160 blocks 간격이다. contour가 읽는
     block-height relief는 effective `-1024..0..2048` block scale을 사용한다. 이는 현재
     `combined_macro_height` 분포를 크게 확대해 contour와 heightfield 계단을 실험적으로 진단하기
     위한 값이다. 4-block step은 수백 level과 수백만 segment를 쉽게 만들기 때문에, 기본값은 성능과
     판독성을 우선해 더 성긴 32-block contour로 둔다.
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
-> river plan cache
-> final cell context cache
-> boundary cache
-> macro field tile cache
-> heightfield / water surface cache
-> chunk generation samples column/window data
-> voxel fill writes ChunkData
```

chunk fill은 매 column마다 가장 가까운 ridge curve, river curve, coast curve를 직접 다시 찾지 않아야
한다. worker/cache miss에서 `MacroFieldTile`을 준비하고, chunk generation은 필요한 column/window만
sample한다.

launch 구현은 ridge/coast/river influence를 per-sample full curve scan 대신 tile-local raster pass로
굽는다. ridge/coast는 source pixel과 distance propagation으로 envelope를 만들고, river는 river plan의
selected edge가 가리키는 canonical noisy curve를 segment-local tile guide로 굽는다.
ownership side 판정은 아직 per-sample query로 남아 있는데,
이것은 visible mask boundary 정확도를 지키기 위한 보수적 선택이다. site bucket lookup은 sample fill
hot path에서 후보 `Vec`을 만들지 않고 bucket window를 직접 순회해야 하며, nearest polyline query는
제곱거리로 후보 segment를 고른 뒤 최종 distance만 계산해 같은 결과를 더 적은 scalar work로 만든다.
4K preview의 남은 주된 비용은 ownership side query이며, 후속 최적화는 owner classification field를
같은 tile cache에 굽는 것이다.

---

## Preview

프로젝트 요구상 각 generation stage는 topdown preview binary로 확인 가능해야 한다.

`macro_field` preview는 최소한 아래 channel을 각각 2D로 출력할 수 있어야 한다.

- macro elevation
- ocean/coast/lake/dry basin mask. coast/lake/ocean 경계는 noisy boundary를 따라 보여야 한다.
  dry basin은 별도 mask/color로 표시되며 coast 노란색과 구분되어야 한다.
- ridge influence
- river valley strength/distance/flow hint와 river bed hint. river plan의 broad valley guide가 보여야
  하며, 이 guide는 tile influence raster pass 결과를 사용한다. 상류는 좁고 급한 valley, 하류는 넓고
  완만한 valley로 보여야 한다. 좁은 river bed 외곽이 combined height에서 두꺼운 blob처럼 보이면 회귀다.
- combined macro height. broad river valley가 Perlin 전 높이에 반영되어야 하며,
  preview 색상은 진단용 heat map이 아니라 muted blue-gray, green-gray, olive/gray, pale gray로 이어지는
  subtle terrain ramp를 사용해 pre-Perlin topdown 지형 표면처럼 읽혀야 한다.
- final cell context / biome influence. stage 8에서 resolve된 final temperature, hydration,
  hydrology role, water proximity, rain shadow, biome influence를 보여주며, macro_field가 biome을
  재결정하지 않았음을 metadata/source note로 기록해야 한다.
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
combined macro height preview: -0.50 .. 1.00
```

per-image min/max와 robust percentile은 metadata/stdout 진단값일 뿐 color scale의 source가 아니다.
모든 channel은 canonical noisy Voronoi graph edge overlay를 기본으로 표시해야 한다. 사용자가
terrain tile 경계를 확인한다고 말할 때의 1차 의미는 macro-field cache grid가 아니라, stage 9의
`BoundaryCache`가 제공하는 noisy edge geometry다. 이 overlay는 field 값을 가리지 않는 faint
reference layer여야 하며, 기본 alpha는 강한 선 레이어가 아니라 위치 확인용 수준이어야 한다.
straight nearest-site 경계가 아니라 canonical noisy curve를 따른다.

macro field preview는 selected river centerline도 `RiverPlan` segment와 `BoundaryCache` canonical
curve를 기준으로 표시해야 한다. 이 선은 river valley field의 폭을 대체하지 않는 진단용 중심선이며,
본류성 reach는 지류보다 조금 더 두껍게 그려도 된다. preview metadata/stdout은 선택된 river-plan
segment 수와 실제 clipping 후 그려진 centerline polyline segment 수를 기록해야 한다.
centerline 위에는 selected hydrology의 `GraphDrainageNodeKind::Source` 중 outgoing selected segment가
있는 node를 circular source marker로 표시한다. downstream selected path가 confluence에 먼저 닿는
source는 tributary marker(amber/yellow ring), terminal/coast/lake endpoint에 먼저 닿는 source는
mainstem marker(bright cyan/white ring)로 구분한다. marker는 river centerline보다 위, legend/scale
bar/compass보다 아래에 그려야 하며 metadata/stdout에는 mainstem source marker 수, tributary source
marker 수, 실제 viewport 안에 그려진 marker 수를 기록한다.

모든 macro field preview output은 방향 compass overlay를 포함한다. 기준은 world topdown 좌표계이며
이미지 위쪽은 북(N), 오른쪽은 동(E), 아래쪽은 남(S), 왼쪽은 서(W)다. compass는 legend와 scale bar를
가리지 않는 보조 overlay여야 하고 field channel 값을 바꾸지 않는다.

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

1. `macro_field`는 graph/macro/hydrology/river-plan/final-cell-context/boundary를 대체하는 source of truth가 아니다.
2. Perlin micro relief는 `macro_field` 이후에 합성되며 macro ownership을 뒤집으면 안 된다.
3. ridge guide는 edge maxima skeleton이고, ridge influence는 heightfield가 읽을 수 있는 주변 envelope
   진단 channel이다. 현재 launch slice에서는 ridge raise가 combined height에서 disabled 상태다.
   broad mountain elevation model 없이 narrow ridge envelope만 높이에 더하면 1블록 contour 기준에서
   pinpoint maxima와 불연속적인 등고선 밀도 변화를 만들기 때문이다.
   Perlin 전 단계에서 ridge influence가 거의 모든 tile sample에 nonzero low-level grain으로 깔리면
   안 된다. ridge를 높이로 재도입할 때는 guide 위 한 점만 밝은 pinpoint로 남지 않고, selected ridge
   path를 따라 연결된 mountain belt shoulder가 보여야 한다.
4. river valley는 hydrology selected segment를 번역한 river plan만 읽어야 하며, macro river candidate를 강으로 해석하면 안 된다.
5. river morphology는 river plan의 reach parameter를 따라야 한다. broad valley width/depth와 narrow
   bed width/depth는 selected/display flow와 reach type에 비례해야 하며, 고정 폭 corridor를 모든 강에
   적용하면 안 된다. macro_field combined height는 broad valley를 주로 반영하고, narrow river bed를
   강하게 직접 파서 bend blob을 만들면 안 된다.
6. tile sample fill은 deterministic해야 하며, 병렬 scheduling이 sample 순서나 값에 영향을 주면 안 된다.
7. combined macro height는 finite 값이어야 하고 preview 가능한 범위를 유지해야 한다.
8. dry basin은 water mask가 아니며, combined macro height에서 lake/ocean flatten을 적용하지 않는다.
9. dry basin 또는 lake/wetland와 land 사이의 owner boundary는 coast mask를 만들지 않는다. coast
   mask는 connected ocean basin과 non-ocean terrain 사이의 explicit coast context만 읽는다.
10. preview renderer는 macro field tile 내부를 local low/high로 정규화하지 않고, 문서화된 absolute
   normalized scale을 사용해야 한다.
11. contour segment는 preview/debug layer이며, source graph/macro/hydrology/river-plan/final-cell-context/boundary나 heightfield
    scalar source를 대체하지 않는다. heightfield가 contour-guided mode를 사용할 때도 같은 level/step
    domain을 공유할 뿐, contour polyline을 새 terrain source로 삼지 않는다.
12. explicit coast boundary는 coast mask/influence source로 남지만, elevation은 local macro_map
    source interpolation field를 읽는다. macro_field가 connected ocean과 non-ocean terrain 사이에
    별도 coast-specific height profile이나 hard owner plateau를 적용하면 회귀다.

---

## 현재 구현 상태

- `src/world/generation/macro_field/mod.rs`가 `MacroFieldTileConfig`, `MacroFieldSample`,
  `MacroFieldTile`, `generate_macro_field_tile`을 제공한다.
- sample fill은 rayon parallel iterator를 사용하고, index 기반 위치 계산으로 deterministic order를 유지한다.
- launch rasterizer는 nearest macro site를 기본 lookup으로 사용하되, boundary blend radius 안에서는
  canonical noisy boundary curve의 side test로 owner/mask boundary를 고른다. macro elevation은
  가까운 `MacroSite.signed_macro_elevation` source들을 local distance-weighted interpolation으로
  샘플한다. site 중심 값은 그대로 보존하고, owner switch만으로 scalar가 계단처럼 끊기지 않아야 한다.
  현재 보간은 fixed top-K truncation 없이 compact support radius 안의 후보 전체를 사용한다. 후보가
  radius에 들어오고 나갈 때 weight가 0으로 수렴해야 하며, site bucket 경계나 K-nearest 후보 교체선이
  visible straight height step으로 나타나면 회귀다. 내부 local maxima가 과밀해지지 않도록 interpolation
  radius는 compact하게 유지하고 inverse-distance power는 가까운 source가 더 우세하도록 둔다.
- site spatial lookup은 각 site가 하나의 bucket에만 들어간다는 전제 아래 per-sample candidate
  allocation/sort/dedup을 하지 않고 bucket window를 직접 순회한다. curve lookup은 하나의 curve가 여러
  bucket에 들어가므로 기존처럼 candidate dedup을 유지한다. curve lookup bucket은 site lookup bucket보다
  작게 유지해 noisy-boundary side query가 불필요하게 많은 curve 후보를 검사하지 않게 한다.
- boundary/polyline nearest queries compare squared segment distances first and only take the final
  square root for the selected nearest distance. This preserves deterministic nearest-segment semantics
  while reducing repeated scalar work in sample fill.
- macro_field는 terrain-kind-specific scalar height boundary blend를 소유하지 않는다. owner/mask
  판정은 전체 noisy boundary grid를 계속 사용하지만, source scalar는 macro_map site elevation들의
  local interpolation을 읽는다.
- sample fill 뒤에는 disconnected ocean-owned raster component를 water mask에서 제거한다. 이 guard는
  noisy owner side query가 해안 land 안쪽에 만든 고립 `CoastOcean` 파편을 downstream water로 확정하지
  않기 위한 최소 후처리이며, threshold는 sample spacing에 맞춘 block area 기준으로 계산한다. tile edge
  ocean과 `OceanBasin` source를 포함한 큰 detached ocean component, lake/wetland mask는 보존한다. 타일
  안의 유일하거나 가장 큰 ocean component라도 `CoastOcean`-only이고 tile edge에 닿지 않으면 제거한다.
- `biome_context`와 `biome`은 macro_map이 resolve한 nearest site `GraphBiomeCell`을 전달한다. 이
  단계는 biome을 다시 분류하지 않고, macro_map stage 끝의 graph-first classification을 cache sample에
  싣는다.
- ridge/coast influence는 selected guide edge의 canonical noisy curve를 tile source pixel로 rasterize한
  뒤 chamfer distance field로 만든다. river influence는 `RiverSegmentPlan`이 참조하는 selected edge id의
  canonical noisy curve를 anti-aliased corridor로 굽는다. corridor width와 bed-depth hint는 fixed radius나
  flow hint만으로 재추정하지 않고 river plan의 `broad_valley_width_blocks`와 `bed_depth_blocks`를 읽는다.
  subpixel coverage 기반 valley strength, nearest distance, blended flow hint, 단순 bed/roughness/gravel
  diagnostic hint를 저장한다. 같은 connected river component 안의 overlapping broad strokes는
  soft union으로 strength/hint를 합성하지만, 다른 component가 이미 더 가까운 sample은 덮어쓰지 않는다.
  이 제한은 confluence/joint cusp를 줄이면서 가까운 독립 하천을 하나의 blob corridor로 병합하지 않기
  위한 launch-scope guard다. 기본 `river_carve_scale`은 shared block-height domain에서 broad-valley
  lowering이 과도하게 깊어지지 않도록 `0.018`이며, 낮은 flow에서는 이 값의 작은 일부만 적용한다. 실제
  narrow bed depth는 combined height에 직접 과하게 새기지 않고 heightfield/water/surface stage가 읽는
  hint로 남긴다.
- lake/wetland lowering은 hard lake ownership mask가 아니라 noisy lake boundary 거리 기반 lowering
  factor로 양쪽에서 연속 전이한다. dry basin mask/statistics는 유지하지만 별도 dry-basin floor/rim
  height profile은 적용하지 않는다.
- lake/wetland boundary lowering factor에도 같은 계열의 작은 world-space roughness offset을 적용해
  lake rim이 지나치게 smooth한 curve로 보이지 않게 한다. `lake_mask` ownership channel 자체는 hard
  source로 유지한다.
- signed polygon containment와 더 정교한 multi-edge blend는 후속 단계에서 확장할 수 있지만,
  visible macro field boundary가 straight nearest-site raster로 되돌아가면 회귀다.
