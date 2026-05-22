# macro_field

## 역할

`macro_field`는 graph-first generator의 11단계인 Voronoi-derived macro/meso field cache를 소유한다.

이 모듈은 새 지형 의미를 다시 noise로 생성하지 않는다. source of truth는 앞 단계의
`VoronoiGraphPatch`, `GraphMacroMap`, `RiverPlan`, final cell context,
`BoundaryCache`, `MesoFeaturePlan`이며, `macro_field`는
그 vector/graph 결과를 heightfield, preview, chunk column sampler가 빠르게 읽을 수 있는 raster/scalar
tile로 굽는다.

즉 이름은 `noise map`이 아니라 graph-derived field cache다. Perlin/fBM micro relief는 이후 단계에서
이 field 위에 더해지는 표면 디테일이다.

---

## 책임

- tile bounds, resolution, world-space sample spacing 계약 정의
- macro elevation, water/coast/lake/dry basin mask, ridge influence, river valley field, river bed hint, meso contribution, final cell
  context/biome influence channel 정의
- noisy boundary 이후의 canonical curve geometry를 읽어 ridge/coast/river distance envelope를 계산하되,
  chunk/preview sample마다 모든 curve 후보를 반복 탐색하지 않도록 tile 단위 influence pass를 먼저 만든다.
- `MesoFeaturePlan`을 tile-local sample channel로 rasterize하고, meso raise/carve/flatten/roughness를 combined height에 반영
- combined macro height를 만들어 heightfield 합성 전의 macro+meso 지형 형태를 제공
- combined macro height를 block-space로 해석한 contour 진단 layer 제공
- chunk fill hot path가 graph/macro/hydrology/river-plan/final-cell-context/boundary/meso-feature를 직접 재탐색하지 않도록 중간 cache surface 제공
- stage preview binary가 각 channel과 combined height를 2D topdown map으로 뽑을 수 있는 데이터 제공

---

## 비책임

- Voronoi graph 생성
- macro ownership/elevation resolve
- hydrology routing 또는 selected river 결정
- river reach type, broad valley/bed width/depth morphology 결정
- noisy boundary curve 생성
- meso feature 후보/anchor/footprint 선택
- Perlin micro relief 생성
- final water surface solve
- biome resolve
- material/vegetation/voxel fill

---

## River Responsibility Map

- `RiverPlan` / `hydrology`: topology, selected river decisions, canonical Q/display discharge,
  reach type, broad-valley width/depth, and narrow bed/water morphology source.
- `macro_field`: selected `RiverPlan` geometry를 tile-local raster/cache influence로 굽고, broad
  valley lowering, terminal 하구 fan guide, downstream hint channel만 제공한다. hydrology routing,
  selected river 수정, final water surface solve, voxel/material output은 소유하지 않는다.
- `heightfield`: macro_field/pixelize가 넘긴 river-resolved height를 block column으로 소비하고,
  integer water hints와 tile-local smoothing만 적용한다. river bed/bank/valley carve를 다시 풀거나
  최종 material/voxel channel을 확정하지 않는다.

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
    &MesoFeaturePlan,
    MacroFieldTileConfig,
) -> MacroFieldTile
```

현재 launch 구현은 아직 concrete meso producer가 연결되지 않았을 수 있다. 그 경우에도 target API와
cache contract는 neutral/empty `MesoFeaturePlan`을 입력으로 두고, macro_field가 meso channel을
0 contribution으로 굽는 방향을 따른다.

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
    river_core_strength,
    river_shoulder_strength,
    river_valley_strength,
    river_distance_blocks,
    river_flow_hint,
    river_longitudinal_blocks,
    river_bed_depth_hint,
    river_bank_roughness_hint,
    river_gravel_hint,
    river_cutbank_hint,
    estuary_water_strength,
    estuary_water_depth_hint,
    meso_raise_strength,
    meso_carve_strength,
    meso_flatten_strength,
    meso_roughness_strength,
    meso_delta_blocks,
    meso_material_hint,
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

## Implementation File Layout

- `mod.rs`: public facade, tile generation orchestration, sample assembly, and end-to-end orchestration tests.
- `types.rs`: public tile config, sample, tile, stats types, defaults, and config validation.
- `contour.rs`: public contour data, block-height conversion, and Marching Squares extraction helpers.
- `height.rs`: combined/ocean/lake height policy plus shared smoothing and boundary roughness math.
- `ocean.rs`: isolated raster ocean-fragment pruning and ocean sample cleanup.
- `context.rs`: raster context, site/biome/boundary owner lookup, junction handling, and spatial index grids.
- `influence.rs`: ridge/coast/river tile-local influence raster passes and influence-derived tile stats.
- `river.rs`: river width, strength, roughness, bed hint, and polyline distance helpers.
- `test_support.rs`: `cfg(test)` shared fixtures for macro-field leaf and orchestration tests.

`mod.rs` keeps the public `world::generation::macro_field::*` surface stable by re-exporting the public leaf
types/functions. Leaf-only helpers use parent-module visibility because they are implementation details of the
single macro-field stage, not cross-module contracts.

---

## 처리 순서

tile 생성은 먼저 빈 sample grid와 feature influence raster를 만든 뒤, 각 world-space sample point를
병렬로 채운다.

1. tile origin, width, height, sample spacing으로 world-space `(x, z)`를 계산한다.
2. ridge, coast, river plan guide를 tile-local influence field로 rasterize한다.
   - ridge/coast는 launch 성능을 위해 source pixel과 chamfer distance propagation을 계속 사용할 수 있다.
   - river는 stage 7 `RiverPlan`의 selected edge id와 flow hint를 읽고, 해당 edge의 canonical noisy
     curve를 topology/guide로 사용한다. raster distance를 계산할 때는 endpoint를 보존한 river-only
     rounded realization path를 만들고, 내부 kink는 flow/width에 비례해 완만하게 당긴다. 따라서 강은
     Voronoi edge를 기준선으로 삼되 매 sample column이 noisy edge의 각진 segment를 그대로 따르지는 않는다.
   - 각 sample cell은 실제 물/강바닥 corridor인 `river_core_strength`와 broad valley context guide인
     `river_shoulder_strength`를 분리해 보존한다. `river_valley_strength`는 기존 preview/tool 호환을 위한
     legacy aggregate diagnostic이며 water/bed eligibility의 source가 아니다. combined height에는
     `river_shoulder_strength` 기반 contextual valley modulation과 `river_core_strength` 중심부의 bed
     downcut을 함께 반영한다. nearest rounded river
     source polyline의 누적 arc length에서 결정적인 `river_longitudinal_blocks` hint도 함께 저장하지만,
     이 값은 downstream diagnostic/hint이며 combined height에 직접 noise/bias로 더하지 않는다. segment나
     confluence ownership이 바뀌는 곳에서 longitudinal hint가 hard switch하면 1-block contour가 강
     진행방향과 수직인 직선 seam처럼 읽히기 때문이다. broad valley height는 source macro elevation을
     보존하되, river boundary shape 전체를 균일하게 내리는 fixed floor보다 projected centerline과 주변
     source elevation의 상대 relief를 우선해 낮춘다. 그래서 combined contour는 river를 가로지르는 cut
     mark가 아니라 river axis와 함께 눕는 낮은 골짜기 맥락으로 읽혀야 한다. narrow bed의 중심부 downcut과
     deterministic non-uniformity도 이 단계에서 baked source height로 들어가야 하며, heightfield가 이를
     다시 파면 안 된다.
   - 이 구조의 목표는 기존 `O(samples * candidate curves * curve segments)` distance query를
     `O(curve source rasterization + samples)` 계열의 bounded tile pass로 바꾸는 것이다.
   - 현재 launch 구현은 ridge/coast/river influence를 이 raster pass로 처리한다.
   - river anti-aliased bake는 selected river segment의 canonical curve 선분들을 굽고, endpoint를
     공유하는 river source들을 하나의 connected raster component로 묶는다. 같은 component 안에서도
     overlap strength를 additive하게 키우지 않고 component-local max/nearest ownership으로 합성해
     bend/joint 주변의 pointed cusp를 줄이되 원형 blob처럼 부풀지 않게 한다. 서로 endpoint를 공유하지
     않는 가까운 river component끼리는 nearest local ownership을 유지해 독립적인 평행 하천이 하나의 넓은
     corridor로 합쳐지지 않게 한다.
   - river raster pass 뒤에는 bounded concave-cusp cleanup을 같은 macro field cache 안에서 수행한다.
     near-threshold sample은 이미 dense river neighbors와 orthogonal support를 가진 경우에만 river
     water/core threshold까지 승격한다. 이 후처리는 raster strength만 보정하며 selected hydrology,
     river_plan geometry, macro masks를 바꾸지 않고 convex outside bank corner는 보존해야 한다.
3. owner/mask 판정은 raw nearest macro site가 아니라 canonical noisy boundary curve set의 nearest
   side query를 기준으로 고른다. 각 sample은 가까운 `NoisyBoundaryCurve`의 양쪽 owner site 중 curve
   side와 일치하는 site를 visible owner로 사용한다. raw nearest macro site는 boundary candidate가 전혀
   없을 때의 fallback일 뿐이며, surface material cell을 나누는 기준으로 쓰면 안 된다.
   `boundary_blend_radius_blocks`와 curve displacement amplitude는 material/lake lowering transition
   폭이나 curve 생성 guard이지, noisy owner 판정의 최대 거리로 쓰면 안 된다.
   - 단, sample point가 `BoundaryCache`의 rounded junction radius 안에 있으면 단일 nearest edge side
     판정보다 junction 판정을 우선한다. 이때 owner는 해당 corner에 incident한 macro site 중 sample과 가장
     가까운 site다. incident site set이 비어 있거나 macro_map site를 찾지 못하면 nearest boundary-side
     flow로 fallback한다. 이 정책은 triple/multi-edge corner에서 pointed wedge를 줄이기 위한
     sampling rule이며 raw graph topology, curve anchors, edge ids를 바꾸지 않는다.
   - 이 단계의 visible ownership/mask boundary는 straight nearest-site 선이 아니라 stage 9
     `BoundaryCache`의 `NoisyBoundaryCurve`를 따라야 한다.
   - noisy curve side 판정은 가장 가까운 polyline segment의 sign을 그대로 쓰지 않는다. anchor chord와
     noisy curve가 만드는 displacement ribbon 안에서는 anchor-side classification을 한 번 toggle해,
     굴곡진 polyline의 segment medial axis가 material boundary처럼 보이지 않게 한다.
   - surface material과 terrain owner/mask channel은 raw nearest-site line이나 curve amplitude band
     edge에서 갈라지면 회귀다. 서로 다른 non-water material이 만나는 transition은 그 두 owner site를 잇는
     canonical noisy boundary curve 가까이에 있어야 한다.
   - macro elevation scalar의 source는 `macro_map`의 `MacroSite.signed_macro_elevation`이다.
     `macro_field`는 가까운 macro site source elevation을 짧은 범위에서 보간해 continuous scalar field를
     만든다. site 중심에서는 원본 elevation을 그대로 보존해야 하며, 장거리 land-group 평균이나
     terrain-kind-specific profile을 만들면 안 된다. 보간 candidate를 fixed top-K로 자르면 K번째/다음
     site의 higher-order Voronoi boundary가 xz 평면에서 직선 단차로 보일 수 있으므로, compact support
     falloff가 0으로 수렴하는 radius 안의 site 전체를 사용해야 한다.
   - boundary owner-side query 안에서도 scalar height와 owner/mask 판정을 분리한다. owner/mask 판정은
     여전히 가장 가까운 noisy boundary side query를 따르지만, scalar height는 선택된 owner site의 hard
     value가 아니라 같은 local interpolation field를 읽는다. explicit coast boundary는 coast
     mask/influence source로 남지만, macro_field 단계에서 coast-specific elevation profile을 별도로
     적용하지 않는다. coast/land owner sample의 급경사가 보이면 macro_map source elevation 또는 이후
     stage로 진단해야 하며, macro_field가 숨기거나 보정하지 않는다.
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
   - macro_field가 combined height에 반영하는 값은 `river_shoulder_strength` 기반 valley context
     modulation과 `river_core_strength` 기반 center bed downcut이다. 이 modulation은 nearest rounded
     centerline projection의 source elevation context와 flow-scaled shoulder/core strength를 읽고,
     `river_longitudinal_blocks`는 downstream diagnostic/hint로만 보존한다. global x/z projection이나
     edge-local arc length를 combined height의 직접 floor-noise source로 쓰지 않으므로 broad shoulder
     등고선이 river를 가로지르는 반복 slab/band로 고정되면 회귀다. 좁은 river bed 중심부, U/V 단면의
     깊이감, roughness/gravel 기반의 작은 비균일성은 이 단계에서 source height로 baked되어야 한다.
     core downcut은 Q와 `river_bed_depth_hint`에서 먼저 하나의 target depth budget을 만들고,
     low-Q에서는 center로 급히 모이는 좁은 V profile, high-Q에서는 중간 단면도 거의 같은 깊이를 갖는
     넓은 U profile로 섞는다. 같은 depth budget의 절반은 normalized shoulder cap을 읽는 inner
     bank/riverbed profile에 baseline으로 적용해, 물 가장자리와 강둑이 core와 같은 계열로 낮아지되
     broad valley 전체가 균일하게 내려앉지 않게 한다. low-Q depth budget은 별도 guard로 줄여
     headwater가 깊은 단층처럼 패이지 않아야 한다.
     downcut budget은 river별로 완전히 같은 strength를 쓰지 않고, world-space coherent noise와
     bank roughness로 짧고 긴 스케일의 deterministic variation을 받는다. rounded river segment의
     local bend sign을 읽어 bend 안쪽은 `river_gravel_hint`를 키워 덜 깊게 깎이는 gravel-bar 성향을
     주고, 바깥쪽은 `river_cutbank_hint`를 키워 조금 더 깊게 깎이는 cutbank 성향을 준다. 이 값들은
     topology나 river width를 바꾸는 source가 아니라 combined height lowering scale만 조절하는
     morphology hint다.
   - `river_core_strength`는 downstream heightfield/water policy가 읽는 0..1 water/bed corridor profile이다.
     high-core 폭은 river_plan의 absolute `bed_width_blocks`를 full water-width target으로 읽는다.
     `river_shoulder_strength`는 broad valley context profile이며 `broad_valley_width_blocks`를
     source guide로 읽되, macro_field 단계에서는 high-Q/downstream reach의 non-core shoulder 반경을
     logarithmic growth curve로 압축하고 planned downstream scale의 대략 절반 근처에서 cap한다.
     raster 단계에서 Voronoi cell 크기를 다시 읽어 동적으로 폭을 재계산하지 않는다.
     이 cap은 shoulder/context에만 적용하며, `river_core_strength`가 읽는 water/bed corridor의
     absolute `bed_width_blocks` target은 바꾸지 않는다.
     low-Q/headwater shoulder profile은 같은 좁은 반경 안에서 immediate shoulder falloff와 cap을 조금
     더 살리되, downstream high-Q cap 끝값은 유지한다.
     combined height에 반영되는 shoulder context는 river shoulder strength 전체를 연속 감쇠로 읽는다.
     낮은 broad-tail 값도 hard cutoff로 0 처리하지 않는다. cutoff boundary가 생기면 block-height contour가
     river 진행 방향과 무관한 직선 onset seam처럼 읽히기 때문이다. shoulder lowering은 source macro
     elevation 자체를 보존한 채 relief compression과 stronger centerline pull 중심으로 감산한다. lowland/near-sea
     floor bias는 source relief 또는 projected centerline drop이 있을 때만 매우 약하게 들어가며, high-Q shoulder
     height modulation도 capped logarithmic profile을 읽어 broad valley가 river boundary shape 그대로 균일하게
     내려앉지 않게 한다. macro_field의 broad shoulder lowering은 heightfield에서 다시 파지 않아도
     valley로 읽힐 만큼 충분히 들어가야 하지만, shoulder strength 변화가 source relief를 상쇄할 정도로
     균일하게 깊어지면 contour slab/vertical seam이 생긴다. centerline은 valley 방향성 hint일 뿐
     cross-section을 평평하게 만드는 target height가 아니다. 단, sea-level 근처 source는 river
     mouth/coast continuity를 위해 작은 추가 bias를 받을 수 있다.
   - river raster pass는 shoulder/core sample마다 가장 가까운 rounded centerline projection과 river
     chain 누적 arc length 기반 `river_longitudinal_blocks`도 보존한다. 이 값은 downstream hint로 유지되지만
     combined height의 직접 floor-noise source가 아니다. 같은 connected river component 안에서 넓은 shoulder
     stroke가 겹치는 sample은 source 하나의 nearest longitudinal 값으로 hard switch하지 않고,
     component-local contribution weight로 longitudinal hint를 섞어 confluence/joint 주변 diagnostic seam을
     줄인다. broad shoulder 안의 combined height는 강을 새로 routing하거나 RiverPlan geometry를 바꾸지 않고,
     centerline context와 flow-scaled shoulder strength로만 낮아져야 한다.
   - water/core boundary에는 world-space deterministic roughness offset을 작게 적용한다. 이 offset은
     selected river curve나 hydrology topology를 새로 만들지 않고, river_plan이 정한 water radius 주변의
     threshold band에서만 거리 profile을 흔든다. roughness amplitude는 planned water width와 flow hint로
     제한해 headwater가 과하게 넓어지지 않게 하며, broad valley shoulder 밖에서는 0으로 fade되어
     valley topology나 서로 다른 river component ownership을 바꾸지 않는다.
   - 상류 broad-valley context modulation은 land/broad valley가 과하게 넓게 파이지 않도록 낮은 Q에서 좁은
     shoulder로 적용한다. 이 조정은 combined macro height의 broad valley width/profile을 줄이며,
     center bed downcut과 river bed/water depth hint는 유지한다.
   - 깊이 정보는 diagnostic hint로도 전달하지만, 현실적인 단면 carve의 terrain-height source는
     macro_field combined height다. heightfield/water/surface 단계는 이 source를 소비하고 water/material
     출력을 정리할 뿐 같은 Q를 이용해 terrain을 다시 파지 않는다.
8. final cell context를 sample 위치에 맞춰 raster/cache한다.
   - final temperature, final hydration, hydrology role, water proximity, rain shadow, biome influence는
     stage 8에서 이미 resolve된 값이다.
   - macro_field는 biome을 새로 분류하지 않는다. 필요한 경우 boundary/domain-warped blend와 sample
     interpolation을 적용해 downstream stage가 읽을 cache channel로 옮긴다.
   - biome influence가 hard owner straight boundary처럼 보이면 회귀다. visible material 경계는
     surface_plan에서 hydrology role, slope/exposure, dithering과 함께 최종 표현된다.
9. stage 10 `MesoFeaturePlan`을 tile-local sample channel로 rasterize한다.
   - hill/knob/secondary spur 계열은 `meso_raise_strength`와 `meso_delta_blocks > 0`으로 들어온다.
   - ravine/closed hollow/terrace carve 계열은 `meso_carve_strength`와 `meso_delta_blocks < 0`으로 들어온다.
   - terrace/bench 계열은 `meso_flatten_strength`를 통해 local height를 직접 평면화하는 것이 아니라,
     falloff 안의 target height 쪽으로 부드럽게 압축하는 contribution을 제공한다.
   - `meso_roughness_strength`는 Perlin micro relief보다 큰 중간 규모 변주를 뜻하지만, macro ownership,
     selected river, lake/ocean mask를 뒤집으면 안 된다.
   - protected mask에 닿은 feature는 rejection/attenuation diagnostic을 남기고, river/lake/coast
     continuity를 깨는 contribution을 만들면 안 된다.
10. 아래 계열로 combined macro height를 계산한다. 이 단계의 river contribution은 최종 물/복셀
   channel fill은 아니지만, heightfield가 소비할 실제 river valley/core bed terrain source이며 preview에서
   보여야 한다.

```text
combined_macro_height =
    ocean_bathymetry_shape(macro_elevation + ridge_raise + meso_delta)        for ocean-owned samples
    river_shoulder_context(macro_elevation, centerline_elevation)
      - river_core_center_downcut(core_strength, bed_depth_hint, position)
      + ridge_raise + meso_delta
                                                                    for ordinary non-lake samples
    river_shoulder_context(macro_elevation, centerline_elevation)
      - river_core_center_downcut(core_strength, bed_depth_hint, position)
      + ridge_raise + meso_delta
      - lake_u_bed_carve                                         for lake/wetland samples
```

`ridge_height_scale`의 launch 기본값은 `0`이다. 즉 현재 `combined_macro_height`는 ridge guide를
높이 maxima로 직접 더하지 않고, macro elevation과 ocean bathymetry/rivers/lakes 제약만 합성한다.
ridge guide는 여전히 별도 channel과 stats로 확인할 수 있으며, 이후 stage에서 연결된 broad mountain
elevation model을 설계한 뒤 재도입한다.

`combined_macro_height`는 최종 material/voxel fill이 아니다. 하지만 stage 10 meso contribution과 river
valley/core bed morphology까지 이미 bake된 pre-heightfield terrain source다. 이후 heightfield/water surface
composition은 이 값을 읽어 column resolve와 post-process smoothing만 수행한다.

Ocean은 lake flatten을 공유하지 않는다. `OceanBasin`/`CoastOcean` sample은 macro_map에서 넘어온
source `macro_elevation`을 그대로 시작점으로 읽는다. source가 `0` 이상이면 ocean-owned sample이라도
terrain bed를 해수면 이하로 강제하지 않고 source height를 보존한다. 얕은 음수 source도 고정 shallow
plane으로 점프하지 않고 signed source depth에 가깝게 유지되어 coast->sea y continuity를 보존한다.
더 깊은 음수 source만 continental shelf -> continental slope -> ocean basin처럼 읽히는 S-curve
bathymetry로 변환한다. continental shelf는 좁게 유지하고, 그 뒤의 중간
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

11. 필요한 경우 `combined_macro_height`에서 contour 진단 layer를 추출한다.
   - contour 추출은 Marching Squares 기반이다.
   - level은 normalized scalar가 아니라 heightfield 직전 block-height 기준이다.
   - 기본 preview step은 32 blocks, major contour는 5 level마다 160 blocks 간격이다. contour가 읽는
     block-height relief는 effective `-1024..0..2048` block scale을 사용한다. 이는 현재
     `combined_macro_height` 분포를 크게 확대해 contour와 heightfield 계단을 실험적으로 진단하기
     위한 값이다. 4-block step은 수백 level과 수백만 segment를 쉽게 만들기 때문에, 기본값은 성능과
     판독성을 우선해 더 성긴 32-block contour로 둔다.
   - preview contour 색은 height에 따라 달라져야 한다. 낮은 contour는 푸른 계열,
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
-> meso feature cache
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

`macro_field_preview`의 positional input은 `pixelize_preview`와 같은 `<seed> <cx> <cz> <r>` 계약을
따른다. `cx/cz`는 chunk coordinate이고, `r`은 inclusive square chunk radius다. 같은 seed/cx/cz/r과
square output size를 주면 stage 11 macro field, stage 12 pixelize, stage 13 heightfield preview가
같은 chunk footprint를 검사한다. Non-square output에서는 X축 radius footprint를 기준으로 Z축 world
span을 image aspect에 맞춰 줄인다. 이는 현재 `MacroFieldTileConfig`가 X/Z 별도 spacing이 아니라
단일 sample spacing을 갖기 때문이다.

`macro_field` preview는 최소한 아래 channel을 각각 2D로 출력할 수 있어야 한다.

- macro elevation
- ocean/coast/lake/dry basin mask. coast/lake/ocean 경계는 noisy boundary를 따라 보여야 한다.
  dry basin은 별도 mask/color로 표시되며 coast 노란색과 구분되어야 한다.
- ridge influence
- river valley strength/distance/flow hint와 river bed hint. river plan의 broad valley guide가 보여야
  하며, 이 guide는 tile influence raster pass 결과를 사용한다. 상류는 좁고 급한 valley, 하류는 넓고
  완만한 valley로 보여야 한다. 좁은 river bed 외곽이 combined height에서 두꺼운 blob처럼 보이면 회귀다.
- meso contribution. stage 10 `MesoFeaturePlan`에서 온 raise/carve/flatten/roughness/material hint가
  feature footprint와 protected mask를 기준으로 보일 수 있어야 한다.
- combined macro height. broad river valley와 meso contribution이 Perlin 전 높이에 반영되어야 하며,
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
selected confluence marker는 같은 drainage node에 selected incoming segment가 두 개 이상이고 selected
outgoing segment가 정확히 하나일 때 orange ring으로 표시할 수 있다.

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

1. `macro_field`는 graph/macro/hydrology/river-plan/final-cell-context/boundary/meso-feature를 대체하는 source of truth가 아니다.
2. Perlin micro relief는 `macro_field` 이후에 합성되며 macro ownership을 뒤집으면 안 된다.
3. meso feature geometry는 stage 10 `MesoFeaturePlan`이 소유한다. macro_field는 이 plan을 sample
   channel과 combined height에 bake할 뿐 feature 후보를 새로 선택하거나 primary hydrology를 바꾸면 안 된다.
4. ridge guide는 edge maxima skeleton이고, ridge influence는 heightfield가 읽을 수 있는 주변 envelope
   진단 channel이다. 현재 launch slice에서는 ridge raise가 combined height에서 disabled 상태다.
   broad mountain elevation model 없이 narrow ridge envelope만 높이에 더하면 1블록 contour 기준에서
   pinpoint maxima와 불연속적인 등고선 밀도 변화를 만들기 때문이다.
   Perlin 전 단계에서 ridge influence가 거의 모든 tile sample에 nonzero low-level grain으로 깔리면
   안 된다. ridge를 높이로 재도입할 때는 guide 위 한 점만 밝은 pinpoint로 남지 않고, selected ridge
   path를 따라 연결된 mountain belt shoulder가 보여야 한다.
5. river valley는 hydrology selected segment를 번역한 river plan만 읽어야 하며, macro river candidate를 강으로 해석하면 안 된다.
6. river morphology는 river plan의 reach parameter를 따라야 한다. broad valley width/depth와 narrow
   bed width/depth는 selected/display flow와 reach type에 비례해야 하며, 고정 폭 corridor를 모든 강에
   적용하면 안 된다. macro_field combined height는 broad valley와 core bed depth를 모두 반영하되,
   narrow river bed를 원형 blob 집합처럼 직접 찍어내면 안 된다. heightfield가 이 morphology를 다시
   계산하거나 재-carve하면 회귀다.
   terminal coast outlet 하구 fan은 이 river morphology를 이어받는 macro_field-local carve guide일
   뿐이며, hydrology/river_plan selected segment topology를 연장하거나 downstream/coast/ocean cell을
   `River`로 승격하면 안 된다.
7. tile sample fill은 deterministic해야 하며, 병렬 scheduling이 sample 순서나 값에 영향을 주면 안 된다.
8. combined macro height는 finite 값이어야 하고 preview 가능한 범위를 유지해야 한다.
9. dry basin은 water mask가 아니며, combined macro height에서 lake/ocean flatten을 적용하지 않는다.
10. dry basin 또는 lake/wetland와 land 사이의 owner boundary는 coast mask를 만들지 않는다. coast
   mask는 connected ocean basin과 non-ocean terrain 사이의 explicit coast context만 읽는다.
11. preview renderer는 macro field tile 내부를 local low/high로 정규화하지 않고, 문서화된 absolute
   normalized scale을 사용해야 한다.
12. contour segment는 preview/debug layer이며, source graph/macro/hydrology/river-plan/final-cell-context/boundary/meso-feature나 heightfield
    scalar source를 대체하지 않는다. heightfield가 contour-guided mode를 사용할 때도 같은 level/step
    domain을 공유할 뿐, contour polyline을 새 terrain source로 삼지 않는다.
13. explicit coast boundary는 coast mask/influence source로 남지만, elevation은 local macro_map
    source interpolation field를 읽는다. macro_field가 connected ocean과 non-ocean terrain 사이에
    별도 coast-specific height profile이나 hard owner plateau를 적용하면 회귀다.

---

## 현재 구현 상태

- `src/world/generation/macro_field/mod.rs`가 `MacroFieldTileConfig`, `MacroFieldSample`,
  `MacroFieldTile`, `generate_macro_field_tile`을 제공한다.
- sample fill은 rayon parallel iterator를 사용하고, index 기반 위치 계산으로 deterministic order를 유지한다.
- launch rasterizer는 canonical noisy boundary curve set의 nearest side query로 owner/mask boundary를
  고른다. raw nearest macro site는 boundary candidate가 없을 때의 fallback이며, surface material
  boundary의 기준이 아니다. macro elevation은
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
- owner sampling keeps noisy-boundary side classification active for terrain owner/mask channels without
  clamping it to the curve displacement amplitude band. `boundary_blend_radius_blocks` continues to control
  only local transition effects such as lake/wetland lowering.
- owner sampling searches nearby boundary-curve buckets for the nearest canonical noisy boundary side and
  uses that curve's owner pair for the visible owner. If the local bucket search has no candidates it falls
  back to the full boundary set; raw nearest-site lookup is only the final no-boundary fallback.
- curve side classification uses the displacement ribbon between each noisy curve and its anchor chord to
  toggle the anchor side. It must not use per-nearest-segment signs as the owner boundary, because highly
  curved polylines can otherwise create material transitions along segment medial axes away from the curve.
- owner sampling materializes deterministic `BoundaryJunction` influence from `BoundaryCache` once in the
  raster context. Inside each junction radius it chooses the nearest incident macro site before nearest
  boundary side classification; outside that radius the existing noisy boundary-side behavior is unchanged.
  The owner-selection use of this radius is capped to sample-scale local support, so the diagnostic
  `BoundaryJunction` smoothing radius cannot become a broad material-owner patch in one-block surface
  previews.
- macro_field는 terrain-kind-specific scalar height boundary blend를 소유하지 않는다. owner/mask
  판정은 전체 noisy boundary grid를 계속 사용하지만, source scalar는 macro_map site elevation들의
  local interpolation을 읽는다.
- sample fill 뒤에는 disconnected ocean-owned raster component를 water mask에서 제거한다. 이 guard는
  noisy owner side query가 해안 land 안쪽에 만든 고립 `CoastOcean` 파편을 downstream water로 확정하지
  않기 위한 최소 후처리이며, threshold는 sample spacing에 맞춘 block area 기준으로 계산한다. tile edge
  ocean과 `OceanBasin` source를 포함한 큰 detached ocean component, lake/wetland mask는 보존한다. 타일
  안의 유일하거나 가장 큰 ocean component라도 `CoastOcean`-only이고 tile edge에 닿지 않으면 제거한다.
  이때 raster sample의 `surface_kind`가 land/coast로 되돌아가면 stale ocean context가 downstream
  surface policy에 남지 않도록 ocean-role `biome_context.water_role`과 `biome`도 land/coast 의미로
  재분류한다.
- `biome_context`와 `biome`은 macro_map이 resolve한 noisy owner site `GraphBiomeCell`을 전달한다. 이
  단계는 biome을 새로 고르지 않고, macro_map stage 끝의 graph-first classification을 cache sample에
  싣는다.
- ridge/coast influence는 selected guide edge의 canonical noisy curve를 tile source pixel로 rasterize한
  뒤 chamfer distance field로 만든다. river influence는 `RiverSegmentPlan`이 참조하는 selected edge id의
  canonical noisy curve를 topology guide로 읽되, distance/strength bake는 endpoint를 보존한 rounded
  realization corridor를 사용한다. corridor width와 bed-depth hint는 fixed radius나 flow hint만으로 재추정하지 않고 river plan의 absolute `bed_width_blocks`,
  `broad_valley_width_blocks`, `bed_depth_blocks`를 읽는다. 단, broad shoulder raster radius/profile은
  macro_field의 bounded logarithmic shoulder helper를 거쳐 high-Q downstream influence를 planned broad
  valley scale의 대략 절반까지 줄인다. core water/bed radius와 bed depth hint는 이 shoulder cap을 타지 않는다.
  low-flow/headwater profile은 그 좁은 corridor 안에서 immediate shoulder와 core bed-depth hint를 조금 더
  강하게 보존하지만, high-Q downstream broad shoulder cap은 그대로 유지한다.
  river stroke rasterization은 launch 기본 검색 반경 `640` blocks 전체를 segment마다 훑지 않고,
  river plan이 제공한 실제 broad-valley/water width와 roughness guard로 계산한 tile-local active
  radius만 스캔한다. 이 radius 밖의 sample은 strength가 0이므로 결과를 바꾸지 않으면서 dense
  heightfield preview의 river 경계 비용을 bounded pass로 유지한다. river stroke pass는 row 단위로
  병렬 rasterize한 뒤 row-major field로 합성한다. 같은 row 안에서는 component-local nearest/union 규칙을
  유지하고, river boundary roughness noise는 sample당 한 번만 평가해 subpixel coverage 비용을 제한한다.
  subpixel coverage 기반 core strength, shoulder strength, legacy aggregate valley strength, nearest
  distance, blended flow hint, 단순 bed/roughness/gravel diagnostic hint를 저장한다. river water/core
  threshold는 같은 raster pass에서 bounded deterministic
  world-space roughness를 적용해 지나치게 매끈한 수면 경계를 피하지만, selected edge path와 broad
  valley guide는 그대로 유지한다. 같은 connected river component 안의 overlapping broad strokes는
  component-local max/nearest ownership으로 strength/hint를 합성하며, 다른 component가 이미 더 가까운
  sample은 덮어쓰지 않는다. 이 제한은 confluence/joint cusp를 줄이면서 가까운 독립 하천을 하나의 blob
  corridor로 병합하지 않기 위한 launch-scope guard다. raster pass 이후 bounded concave-cusp cleanup은
  dense near-threshold river holes만 threshold까지 승격하고, convex bank rounding과 source topology를
  보존한다. 기본 `river_carve_scale`은 shared block-height domain에서 shoulder context modulation의
  최대 이동량을 제한하는 작은 scalar다. 기본값은 `0.012`이다. 낮은 flow에서는 직접 감산 depth를 주로
  죽이는 방식이 아니라, river_plan의 좁은 broad-valley width와 raster profile로 valley context 범위를 줄인다.
  기본 river influence radius는 downstream absolute water width와 broad shoulder를 담을 수 있도록
  `640` blocks다. 실제 narrow bed depth는 core center profile을 통해 combined height에 반영하고,
  같은 bed-depth 값은 heightfield/water/surface stage가 읽는 diagnostic/water-depth hint로도 남긴다.
  selected river chain이 `CoastOutlet` terminal에서 끝나는 마지막 segment는 river topology를
  downstream cell로 연장하지 않는다. 대신 macro_field raster pass 안에서 terminal endpoint 이후
  downstream 방향의 fan/estuary guide를 내부 influence channel로 굽는다. 이 guide는 시작부에서 기존
  terminal river bed/flow width와 이어지고, 진행할수록 lateral half-width가 넓어져 coast/ocean source
  안에서 얕은 shelf 형태로 퍼진다. fan의 시작 반폭과 최종 확산 반폭은 terminal segment의 planned
  water/bed width를 1차 기준으로 삼아, 큰 하류 강의 하구가 기존 수면 폭보다 좁게 pinching되지 않아야
  한다. broad valley width는 보조 확산 context로만 더해진다. low-Q mouth는 fan tail과 최소 downstream reach를 보존해 작은 강도
  coast/ocean source 쪽으로 끊기지 않게 하고, 마지막 river segment가 아주 짧으면 fan 전용 bed-depth
  hint를 낮춰 하구 시작점에서 deep trench target으로 급락하지 않게 한다. fan influence는 terminal
  endpoint 이후의 downstream 누적 거리(`estuary_along_blocks`)도 함께 저장하며, height 합성은 시작부
  허용 depth를 얕게 잡고 거리당 완만하게만 증가시킨다. 따라서 terminal river/core/broad-valley budget을
  다시 조정하지 않고도 짧은 마지막 segment의 estuary 시작부가 바다 shelf 목표 높이로 즉시 snap되지
  않아야 한다. 최종 도달 shelf target도 river carve/depth hint에서 계산한 raw fan depth를 그대로 쓰지
  않고 절반 스케일로 낮춰, 하구 연결부가 과도하게 깊은 trench로 끝나지 않게 한다. deterministic world-space
  roughness는 fan edge만 흔들며 selected
  river segment, hydrology adjacency, surface owner mask를 바꾸지 않는다. 다만 fan 내부에는
  `estuary_water_strength`와 `estuary_water_depth_hint`를 별도로 굽고, strength가 water threshold를
  넘는 above-sea mouth column은 heightfield가 terminal river water surface와 이어진 local river-water
  continuation으로 해석할 수 있다. 이때 water depth hint는 fan bed carve depth와 분리되어, 짧은 terminal
  segment 때문에 bed carve가 slope-limited 되더라도 수면 연결은 기존 river water depth 맥락을 보존한다.
  이 continuation은 selected river segment를 downstream cell로 추가하는 것이 아니라, 하구 y>0 구간의
  수면 단절을 막는 raster hint다. height 합성은 estuary influence를 읽어 coast/ocean near-sea source를
  `combined_macro_height <= 0` 쪽으로 열어 준다. fan edge의 약한 strength는 target depth도 함께 약화해
  경계에서 고립된 water block speckle이나 갑작스러운 한 블록 수면 불일치를 만들지 않아야 한다. lake,
  wetland, dry basin, ordinary inland/no-flow sample은 이 guide의 carve 대상이 아니다.
- lake/wetland lowering은 hard lake ownership mask가 아니라 noisy lake boundary 거리 기반 lowering
  factor로 양쪽에서 연속 전이한다. dry basin mask/statistics는 유지하지만 별도 dry-basin floor/rim
  height profile은 적용하지 않는다.
- lake/wetland boundary lowering factor에도 같은 계열의 작은 world-space roughness offset을 적용해
  lake rim이 지나치게 smooth한 curve로 보이지 않게 한다. `lake_mask` ownership channel 자체는 hard
  source로 유지한다.
- signed polygon containment와 더 정교한 multi-edge blend는 후속 단계에서 확장할 수 있지만,
  visible macro field boundary가 straight nearest-site raster로 되돌아가면 회귀다.
