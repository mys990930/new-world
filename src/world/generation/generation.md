# generation

## 역할

`world::generation`은 새 graph-first 월드 생성 파이프라인을 소유한다.

이 모듈은 기존 `atlas cell` / `chunk` 중심 생성기를 바로 지우지 않고, 새 구조를 단계별로
대체하기 위한 경계다. 런타임 호환이 필요한 동안에는 `src/world/legacy/generation`의 공개 API를
`world::generation::*`로 다시 내보낸다.

---

## 핵심 방향

새 generator는 사각 chunk나 atlas cell을 지형 정체성의 기준으로 쓰지 않는다. Voronoi graph의
site, corner, edge 구조를 macro semantic graph로 사용하고, 최종 높이와 재질은 continuous field,
noisy boundary, heightfield synthesis를 통해 현실화한다.

- graph는 게임플레이와 월드 일관성에 필요한 제약을 담는다.
- noise는 제약으로 고정할 필요가 없는 자연스러운 변주를 만든다.
- chunk는 저장과 출력 window일 뿐, 지형 정체성의 소유자가 아니다.
- runtime chunk fill은 graph/macro/hydrology를 청크마다 새로 만들지 않고, world-owned generation
  cache를 읽어 column/voxel 결과만 합성한다.
- polygon 경계는 후보선이자 소유권 경계일 수 있지만, 그대로 보이는 선이어서는 안 된다.
- 이 프로젝트는 큰 대륙과 바다, 대륙 내부 산맥과 강을 목표로 하되, ocean basin 안에 크고 작은
  섬과 archipelago도 deterministic feature로 허용한다.

이 방향은 Amit Patel의 Polygonal Map Generation 계열 아이디어를 이 프로젝트의 무한 복셀 월드
구조에 맞춰 재해석한 것이다.

참고:

- <https://xenon.stanford.edu/~amitp/game-programming/polygon-map-generation/>
- <https://www.redblobgames.com/maps/noisy-edges/>
- <https://www.redblobgames.com/maps/mapgen4/>
- <https://www.redblobgames.com/maps/terrain-from-noise/>

---

## 책임

- Voronoi graph 기반 macro terrain 생성 단계 소유
- graph region, site, corner, edge 기반의 deterministic 생성 계약
- graph base field 기반 대륙/바다/섬 ownership, macro elevation, 산맥/능선/단층/해안 guide의 생성 순서 정의
- edge 기반 hydrology, watershed, selected river chain과 river morphology plan의 생성 순서 정의
- graph region cache, macro map cache, hydrology/river plan/final-cell-context/boundary/macro field/pixelize/heightfield cache가 chunk fill hot path보다 먼저
  생성되고 공유되는 런타임 계약 정의
- noisy boundary, macro field, chunk pixelize, heightfield/voxel-column realization, surface plan, voxel fill 단계 경계 정의
- 각 단계 이후 topdown preview binary가 접근할 수 있는 stage surface 정의
- 기존 legacy generation API의 임시 호환 re-export

---

## 비책임

- live world storage mutation
- ECS command 해석
- async job scheduling
- GPU preview rendering
- save/load byte format
- 레거시 atlas generator의 내부 정책 변경

---

## 표준 처리 순서

generation pipeline은 chunk를 지형 정체성의 기준으로 쓰지 않는다. chunk는 출력 window이며,
macro terrain identity는 graph와 field가 소유한다.

아래 순서가 현재 graph-first generator의 표준 순서다. 각 단계는 같은 seed, generator version,
area, stage input에 대해 deterministic해야 하며, 단계 직후 topdown preview binary로 검사할 수
있어야 한다. preview output은 공통적으로 방향 compass overlay를 포함하며, macro field 기준으로
topdown preview의 이미지 위쪽은 북(N), 오른쪽은 동(E), 아래쪽은 남(S), 왼쪽은 서(W)를 뜻한다.
단, `heightfield_preview`처럼 quarter/isometric projection을 쓰는 preview는 현재 `--quarter-turns`
투영 후 screen-space에서 N/E/S/W가 놓이는 방향을 표시해야 한다.

1. seed 기반 padded Voronoi/Delaunay dual graph를 생성한다.
2. site/corner에 macro-friendly base field를 부여하고 이웃 graph를 참고해 smoothing한다.
   - `continentality`는 단순 local random 값이 아니라, 대륙성/해양성 site가 장거리로 뭉치는 coherent field여야 한다.
   - `elevation_seed`는 `continentality`와 완전히 독립된 noise가 아니라, land/ocean context와 결합 가능한 macro elevation bias여야 한다.
   - `temperature`, `humidity`도 graph smoothing을 통해 인접 site/corner 사이의 급격한 단절을 줄인다.
3. graph base field를 resolve해 continent/ocean/island ownership과 signed macro elevation을 만든다.
   - `macro_map`은 독자적인 continent/island noise source를 소유하지 않는다.
   - continent/ocean/island의 source of truth는 graph의 smoothed `continentality`와 연결 component 해석이다.
   - 큰 land component는 continent, ocean basin 안의 작은 land component는 island 또는 archipelago로 분류한다.
   - signed macro elevation은 graph `continentality` sea-level threshold에서 `0`이 되도록 만든다.
     `ruggedness`는 같은 continentality 주변의 local relief contrast를 키우며, `elevation_seed`는 이
     local variation을 보조한다.
   - connected ocean coast에 인접한 land owner를 별도 coastal ceiling이나 coast-distance recovery ramp로
     처리하지 않는다. 해안 저지대와 급경사 해안은 threshold 근처 continentality와 ruggedness 차이,
     그리고 macro_map source elevation에서 자연스럽게 나와야 하며, lake/wetland 승격 정책을 과하게
     넓히는 근거가 되어서는 안 된다.
   - water component는 patch/guard boundary 접촉만으로 ocean이 되지 않는다. explicit ocean basin으로
     분류된 장거리 water component와 연결되지 않은 물은 바다에 가까워도 lake/wetland/dry basin 후보로 유지한다.
   - 고립 저지대가 모두 호수가 되어서는 안 된다. 작은 호수는 일반적으로 10 site/cell 이내를 목표로
     하고, 30 site/cell 안팎의 큰 호수는 드문 deep/wet basin 조건에서만 허용하는 soft cap 정책을 따른다.
   - launch 기본 `land_bias`는 기본 preview에서 대략 land:water = 6:4를 목표로 한다. 이 조정은
     graph base `continentality`의 coherent coastline을 그대로 threshold 이동으로 해석하는 것이며,
     단순 직선 coastline을 새로 만들면 안 된다.
   - 1~3 site/cell 규모의 작은 tiny local-minima lake는 river-side 여부와 무관하게 land-owned
     graph 저지대에서 낮은 확률로 생성될 수 있다. 조건은 graph adjacency 기준 더 낮은 land neighbor가
     없는 tiny local-minima component, 낮은 elevation seed, hydration, 낮은 ocean-coastness,
     deterministic component roll을 함께 만족해야 한다. launch 기본 roll은 10,000분의 3,600이고,
     score bonus를 포함해도 10,000분의 4,000을 넘지 않는다. 모든 local minimum을 lake로 승격해서는 안 된다.
4. macro ownership, signed macro elevation, gradient, component context를 읽어 ridge/fault edge guide를 선정한다.
   - ridge는 단순 high elevation edge가 아니라, elevation gradient, land component 내부 위치, ruggedness/mountainness context, drainage divide 가능성을 함께 만족해야 한다.
5. land/ocean ownership 경계에서 coast edge guide를 선정한다.
   - coast는 signed elevation 부호만으로 찾지 않고, connected ocean basin과 land ownership의 경계를 우선한다.
   - lake edge는 macro edge annotation인 `MacroLakeEdgeClass`로 별도 분류한다. 이 class는 인접 site와
     endpoint corner를 함께 읽으며, hydrology의 selected river 금지 기준이 된다.
6. macro elevation, ridge/coast guide, graph topology를 읽어 hydrology를 푼다.
   - 이 단계는 potential guide가 아니라 selected hydrology result를 만든다.
   - downhill, graph-stage local minima, lake/sink/outlet carve, watershed, flow accumulation을 계산한다.
   - local minimum은 lake, sink, outlet carve, dry/closed basin 의미로 분리되어야 하며, local
     minimum이라는 이유만으로 모두 물로 채우지 않는다.
   - selected river chain은 lake/sink/outlet 정책 없이 끊기지 않아야 하며, 최종적으로 ocean outlet 또는 명시적인 lake/sink resolution에 연결되어야 한다.
   - selected ordinary river fragment가 downstream selected path와 valid terminal을 잃으면 hydrology가
     selected geometry에서 제거한다. downstream raw flow ledger는 보존한다.
   - lake로 끝나는 chain과 lake/wetland candidate component로 처음 들어가는 inlet chain은 ocean outlet
     chain과 같은 크기로 취급하지 않는다. raw accumulation은 보존하되, lake 면적/capacity에 비례해
     lake별 top-N incoming chain, inlet raw-flow threshold, 표시/폭 계산용 discharge를 제한한다.
     기본 lake terminal/inlet display discharge는 ocean outlet trunk보다 확연히 낮은 cap을 가진다.
     기준을 통과한 lake-bound chain은 lake edge를 쓰지 않는 범위에서 기존 upstream trunk를 유지하고,
     lake boundary 직전 land-side endpoint에서 `LakeInlet`으로 종료될 수 있다.
   - lake와 river의 접점은 lake edge를 따라 스쳐 지나가는 선이 아니라 `LakeInlet`/`LakeOutlet`
     selected endpoint marker로 표현한다. selected river segment는 lake 내부 edge나 lake boundary
     edge, lake-adjacent edge를 쓰지 않으며, 유입하천은 lake boundary 직전의 land-side endpoint에서 끝나고 유출하천은 같은
     lake component의 다른 boundary vertex 밖 land-side endpoint에서 시작한다. outlet pair는 유입하천의
     land-side approach corner보다 낮고 최소 hop 거리만큼 떨어져야 한다.
   - `LakeInlet`은 충분한 flow accumulation을 가진 selected feeder만 marker로 승격한다. inlet은 lake별
     0개부터 여러 개까지 가능하지만, `LakeOutlet`은 lake별 0개부터 최대 2개까지의 낮고 분리된 후보로
     제한한다. 같은 selected river chain이 lake와 두 번 접촉하면 안 되며, lake inlet에서 끝난 chain과
     lake outlet에서 시작하는 chain은 별도 chain으로 취급한다.
   - lake와 연결된 selected flow endpoint는 반드시 `LakeInlet` 또는 `LakeOutlet` 중 하나로 분류되어야
     하며, 미분류 lake-connected flow는 회귀로 계측한다.
7. hydrology 결과를 river plan으로 번역한다.
   - 이 단계는 물길을 새로 고르지 않는다. source of truth는 stage 6의 selected hydrology result다.
   - selected river adjacency를 stable chain/reach table로 옮기고, downstream progress, display flow,
     raw flow, terminal role을 기반으로 reach type과 diagnostic morphology hint를 제공한다.
   - launch 구현은 selected segment를 추가/삭제하지 않는 chain-local discharge ledger를 제공한다.
     ordinary chain에서는 display discharge와 morphology scale이 downstream으로 줄어들지 않으며,
     lake inlet/outlet은 hydrology의 lake-cap display flow를 존중해 ocean trunk보다 보수적으로 남긴다.
   - river plan은 full hydraulic simulation, U/V 단면 carve, final water surface solve를 하지 않는다.
   - macro_field는 이 plan에서 selected edge와 flow hint를 읽어 canonical noisy curve를 rasterize한다.
     좁은 강바닥 단면이나 하구 fan을 combined height에 직접 새기지 않는다.
8. hydrology와 river plan 결과까지 반영한 final cell context를 resolve한다.
   - 이 단계는 elevation, water proximity, rain shadow, hydrology role을 반영해 final temperature,
     hydration, biome influence를 확정한다.
   - 여기서 말하는 elevation은 final heightfield가 아니라 graph/macro 단계의 signed macro elevation,
     coast distance, ridge/mountainness/ruggedness context, hydrology-selected drainage context다.
   - water proximity는 ocean/coast, lake/wetland, selected river corridor, dry/closed basin을 같은
     water mask로 뭉개지 않고 별도 role로 반영한다.
   - selected `Headwater` river segment에 인접한 land/dry-basin site는 수원지 근처 cell로 보고,
     final hydration을 최소 `0.46`으로 보정한다. 따라서 수원지 바로 옆 final biome은
     `Steppe`, `SemiDesert`, `Desert`, `DryShrubland`, `TemperateGrassland` 같은 건조지대로 남지 않는다.
     이 보정은 hydrology selected result를 읽는 post-hydrology pass이며, pre-hydrology river candidate를
     강처럼 취급하지 않는다.
   - rain shadow는 prevailing wind, mountain/ridge guide, macro elevation gradient, watershed context를
     읽는 deterministic graph/cell-scale field다. 이 단계 뒤에 macro_field가 샘플할 수 있도록 cache
     가능한 cell/edge/corner 또는 tile-independent column context로 남아야 한다.
   - final cell context는 dominant site owner, blended graph influence, final temperature,
     final hydration, hydrology role, water/coast/lake/wetland/dry-basin role, biome influence,
     optional dominant biome id를 함께 제공한다.
   - biome은 이 단계에서 macro_field보다 먼저 resolve된다. macro_field는 biome을 새로 결정하지 않고,
     이미 resolve된 biome influence/context를 raster cache로 옮기거나 heightfield/surface plan이 읽을
     mask/hint로 보존할 수 있다.
   - biome/material visible boundary는 이 단계의 hard owner를 그대로 그리는 것이 아니라 이후 boundary,
     macro_field, surface_plan에서 canonical noisy geometry, gradient, domain warp, dithering을 거쳐
     표현한다.
9. 모든 Voronoi edge를 canonical noisy boundary geometry로 현실화한다. raw graph topology는 그대로 보존한다.
   - boundary stage는 특정 visible feature edge만 골라 curve를 만들지 않는다.
   - `NoisyBoundaryCurve`는 graph edge 전체에 대해 생성되는 `edge id -> noisy polyline/spline` layer다.
   - noisy boundary는 straight edge 위에 sample만 늘리는 것이 아니라 endpoint anchor 사이의 interior
     point를 edge normal 방향으로 흔들어 울퉁불퉁한 visible boundary를 만든다.
   - interior displacement는 sample별 독립 jitter가 아니라 low/mid frequency coherent wave,
     deterministic value-noise knot, smoothing, endpoint falloff를 통해 자연스러운 curve로 만들어야 한다.
   - 기본 amplitude는 4K topdown preview에서 식별 가능해야 하며, 너무 짧은 edge를 제외한 curve가
     거의 직선으로 남으면 회귀다.
   - coast/ridge/fault/lake/ordinary boundary 차이는 curve 존재 여부가 아니라 profile/amplitude/constraint parameter에 반영한다.
   - river는 별도 noisy curve를 만들지 않는다. hydrology selected segment는 edge id path이며, preview, heightfield, water corridor는 해당 edge id의 canonical noisy geometry를 따라간다.
   - lake boundary/internal/lake-adjacent edge에도 noisy curve는 존재하지만, selected river segment가 해당 edge를 타는 것은 계속 금지된다.
10. noisy boundary 이후, macro field 이전에 meso feature plan을 만든다.
    - meso feature는 `hill_cluster`, `upland_knob`, `secondary_spur`, `closed_basin`, `ravine`,
      `terrace` 같은 macro보다 작고 Perlin보다 큰 국소 지형 객체다.
    - 이 단계는 hydrology와 river plan을 읽지만 selected river chain, lake/sink/outlet resolution,
      continent/ocean ownership을 바꾸지 않는다.
    - feature는 world-space anchor, footprint, falloff, priority, seed, height contribution rule,
      protected mask, material/surface hint를 가진 deterministic plan으로 남는다.
    - primary drainage를 바꿀 정도의 feature는 ordinary meso가 아니라 macro/hydrology guide로
      승격해야 한다.
11. graph guide, hydrology, river plan, final cell context, noisy boundary, meso feature plan을 합쳐 macro field tile을 rasterize한다.
    - macro_field는 `MesoFeaturePlan`을 읽어 meso raise/carve/flatten/roughness contribution을
      sample channel과 `combined_macro_height`에 bake한다.
    - `combined_macro_height`는 Perlin 전 결과지만, meso contribution까지 포함한 visible macro/meso
      terrain source다.
12. chunk pixelize 단계에서 `MacroFieldTile`을 chunk boundary에 정렬된 `1 world block = 1 pixel = 1 voxel column` column cache로 변환한다.
    - 이 단계는 graph topology, hydrology, river plan, noisy boundary를 다시 해석하지 않는다.
    - output column은 world x/z, chunk x/z, local x/z, integer `surface_y`, optional integer `water_y`,
      terrain kind hint, source `combined_macro_height`, meso-baked channel, ocean/lake/coast/dry basin/river/ridge mask를 보존한다.
13. heightfield / voxel-column realization은 pixelized column output을 소비한다.
    - 새 path에서 heightfield는 first chunk-aligned pixel resolve를 소유하지 않고, `MacroFieldTile`을 직접
      resample하지 않는다.
    - heightfield는 meso feature geometry를 다시 탐색하지 않고, macro_field/pixelize가 보존한
      meso-baked column 값과 Perlin micro relief를 소비한다.
14. biome/material/water/coast surface plan을 만든다.
    - 현재 graph-first created-world path는 `HeightfieldTile`과 `MacroFieldTile`을
      `generate_surface_plan_area`로 넘겨 biome/material/water/coast surface policy를 만든다.
      vegetation은 아직 생성하지 않는다.
15. vegetation/feature placement plan을 만든다.
    - 현재 launch slice에서는 vegetation placement를 생성하지 않는다.
16. heightfield, water, surface, vegetation plan을 한 번에 `ChunkData`로 voxel fill한다.
    - 현재 구현된 graph-first 저장 경로는 bounded x/z 영역에서 `MacroFieldTile`,
      Perlin-enabled `HeightfieldTile`, `SurfacePlanArea`, `PixelizedChunkArea`를 만든 뒤
      `GraphFirstVoxelPlan`에 surface block policy를 축약 저장한다. `voxelize_graph_first_chunk`는
      water/top/subsurface/base/underwater-top block을 사용해 `ChunkData`를 채운다.

---

## 하위 모듈

현재 구현된 scaffold:

- `graph/graph.md`: graph region, Voronoi site/corner/edge id와 patch 계약
- `field/field.md`: hard owner가 아닌 continuous blended field sampling 계약
- `biome/biome.md`: graph-first final cell biome context와 classification 계약
- `hydrology/hydrology.md`: watershed, drainage node, selected river edge 루트 계약
  - `hydrology/routing.md`: downhill graph, local minima, watershed, raw flow accumulation
  - `hydrology/selection.md`: selected river source 후보와 edge 승격 정책
  - `hydrology/topology.md`: lake contact, selected graph pruning, reachability validation
  - `hydrology/discharge.md`: raw Q, river-system Q, lake-local cap 분리 계약
- `river_plan/river_plan.md`: selected river를 reach morphology와 broad valley / narrow bed plan으로 번역하는 계약
- `pipeline/pipeline.md`: graph-first stage order와 column synthesis scaffold
- `pixelize/pixelize.md`: `MacroFieldTile`을 chunk-aligned `PixelizedChunkArea`로 변환하는 stage 12 구현과 계약

현재 구현된 graph-first processing:

- stage 1 padded Voronoi graph patch 생성: deterministic jittered world-space site 후보를 만들고,
  `delaunator` 기반 Delaunay triangulation을 수행한 뒤, 각 triangle circumcenter를 Voronoi corner로
  삼아 shared Delaunay edge의 양쪽 circumcenter를 연결한다. site/corner/edge 결과는 병렬 계산 뒤
  id 기준 정렬/dedup으로 deterministic order를 유지한다. convex hull의 open edge는 launch 단계에서
  padding/guard 밖 경계로 취급하고, 두 triangle을 가진 interior edge를 안정성 우선으로 노출한다.
- stage 2 base graph field: site raw seed field와 smoothed field를 생성하고, corner field/elevation
  seed를 주변 site 기반으로 안정적으로 계산한다. `continentality`와 `elevation_seed`는 graph preview와
  macro_map이 같은 값을 읽을 수 있도록 장거리 coherent source of truth로 생성한다.
- stage 3/4/5 macro map: 목표 계약상 `macro_map`은 graph patch의 smoothed `continentality`와
  `elevation_seed`를 resolve해 continent/ocean/island ownership, signed macro elevation, coastness,
  mountainness/rugged context, basinness, coast/ridge/fault guide를 별도 annotation layer로 생성한다.
  독자적인 continent/island noise source는 macro_map의 책임이 아니다. 현재 구현은 graph base
  `continentality`를 land/ocean ownership과 signed sea-level elevation의 source of truth로 읽고,
  graph adjacency component로 stage 3 ownership을 resolve한다. coast distance는 coast guide/context
  annotation이며 signed elevation의 inland recovery source가 아니다. 음수 water component도
  connectivity를 읽되 patch/open boundary 접촉만으로 ocean을 만들지 않고, explicit ocean basin
  component에 연결되지 않은 물은 inland lake candidate로 분류한다. stage 4 guide는
  같은 land component 내부성, signed elevation gradient, inlandness, mountainness/rugged context,
  drainage divide potential을 함께 읽어 ridge/fault edge candidate를 선택한다.
  macro_map의 마지막에는 site별 final cell biome context와 classification을 `GraphBiomeCell`로 resolve해
  `macro_field`가 nearest site biome 의미를 함께 전달할 수 있게 한다. ocean biome은 단일 Oceanic이
  아니라 `ShallowOcean`과 `DeepOcean`으로 분리된다.
- stage 6 hydrology: macro guide와 graph topology를 읽어 selected river chain을 확정한다. 현재
  구현은 corner downhill, graph-stage local minimum, outlet carve, watershed, flow accumulation,
  selected river segment를 계산한다. 이 단계의 river는 후보 surface가 아니라
  downhill/local-minimum/outlet 정책을 통과한 결과다.
- stage 7 river plan: selected river segment를 deterministic chain/reach plan table로 번역한다. 현재
  구현은 selected segment set을 보존하면서 selected adjacency를 chain으로 걷고, contiguous same-scale
  segments를 reach로 묶으며, display/raw flow ledger와 보수적인 morphology/diagnostic hint를 제공한다.
  full hydraulic simulation, 복잡한 단면 carve, final water surface solve는 현재 구현 범위가 아니다.
- stage 8 final cell context: graph base field, macro ownership/elevation, coast/lake/ocean/dry basin
  context, selected hydrology role, water proximity, rain shadow를 합성해 final temperature/hydration과
  biome influence를 resolve한다. biome은 macro_field보다 먼저 확정되며, downstream stage는 이를
  재결정하지 않고 cache/sample 가능한 context로 소비한다.
- stage 9 boundary: graph/macro annotation과 final cell context를 읽어 모든 Voronoi edge의 deterministic canonical noisy
  curve layer를 만든다. 구현은 Amit식 noisy edge 원칙을 따라 하나의 Voronoi edge의 두 corner와 두
  site center가 만드는 guard 안에서 midpoint displacement polyline을 생성한다. raw graph topology는
  그대로 남고, selected river는 별도 river curve가 아니라 hydrology segment의 edge id가 가리키는
  canonical curve를 따라 preview/heightfield에서 해석된다.
- stage 10 meso feature: graph/macro/hydrology/river-plan/final-cell-context/boundary cache를 읽어
  macro보다 작고 Perlin보다 큰 국소 지형 feature plan을 만든다. 이 plan은 `ChunkData`나
  heightfield를 직접 수정하지 않고 macro_field가 읽을 deterministic object table이다. ordinary meso는
  selected hydrology를 끊거나 lake/ocean ownership을 바꾸지 않으며, drainage 자체를 바꾸는 feature는
  macro/hydrology guide로 승격해야 한다.
- stage 11 macro field: graph/macro/hydrology/river-plan/final-cell-context/boundary/meso-feature cache를 읽어 tile 단위 raster field를 만든다.
  이 field는 새 noise source가 아니라 pixelize와 downstream heightfield/chunk fill이 읽을 cache다. macro elevation,
  coast/lake/ocean/dry basin mask, ridge/fault influence, river valley, meso contribution, final biome influence,
  combined macro height는 각각
  독립 preview target이어야 하며, combined macro height는 meso contribution이 bake된 Perlin 합성 전 결과를 표시한다. heightfield
  직전 macro field 연속성을 진단하기 위해 block-height 기준 contour preview를 추가로 뽑을 수 있어야 한다.
- stage 12 pixelize: 문서 계약은 `MacroFieldTile`을 chunk-aligned `PixelizedChunkArea` column cache로
  변환하는 새 handoff를 정의한다. 현재 구현은 `generate_pixelized_chunk_area`로 one-block spacing
  macro field tile을 deterministic row-major `PixelizedColumn` sequence로 옮기며, preview는 각
  pixel이 하나의 resolved voxel column인 `pixelize_preview`로 검사한다.
- stage 13 heightfield / voxel-column realization: 현재 구현은 compatibility vertical slice로
  `MacroFieldTile`을 직접 읽어 `HeightfieldTile` column cache로 변환한다. graph-first runtime
  build config는 Perlin micro relief를 기본 활성화해 heightfield column에 반영한다. rewrite target은
  `PixelizedChunkArea` / `PixelizedColumn`을 downstream input으로 소비하는 것이다. heightfield는
  meso feature geometry를 다시 해석하지 않고, pixelize가 보존한 meso-baked source scalar와 integer
  block height contract를 소비해야 한다. Perlin micro relief는 macro ownership을 뒤집으면 안 된다.
- stage 14/15 surface/material/vegetation: graph-first created-world path는 구현된 `surface_plan`을
  호출해 biome/material/water/coast block policy를 만든다. vegetation placement는 아직 생성하지 않는다.
- stage 16 voxel fill: `src/world/generation/voxel/mod.rs`는 `PixelizedChunkArea`를
  surface-aware `GraphFirstVoxelPlan`으로 옮긴 뒤 `voxelize_graph_first_chunk`로 `ChunkData`를
  채운다. 같은 x/z column plan을 vertical chunk stack이 공유하며, `world_create`와 runtime
  create-world job은 이 경로로 bounded dump를 저장할 수 있다.

런타임에서는 위 stage를 chunk마다 반복 실행하지 않는다. `pipeline/pipeline.md`의 runtime cache
contract에 따라 graph region cache, macro map cache, hydrology cache, river plan cache,
final cell context cache, boundary cache, meso feature cache, macro field tile cache, pixelized chunk area cache,
heightfield/voxel-column cache를 worker에서 준비하고, chunk generation은 필요한 world-space
column/window만 sample해 `ChunkData`를 채운다.

문서화된 다음 leaf:

- `river_plan/river_plan.md`: selected river를 reach morphology와 broad valley / narrow bed plan으로 번역하는 계약
- `boundary/boundary.md`: 모든 Voronoi edge의 canonical noisy geometry 계약
- `meso_feature/meso_feature.md`: noisy boundary 이후, macro field 이전의 국소 지형 feature planning 계약
- `macro_field/macro_field.md`: graph-derived signed distance / influence field tile cache
- `pixelize/pixelize.md`: `MacroFieldTile`에서 chunk-aligned resolved column cache로 넘어가는 stage 12 계약
- `heightfield/heightfield.md`: pixelized column output을 소비하는 heightfield / voxel-column rewrite 계약
- `surface_plan/surface_plan.md`: biome, material, water/coast/wetland policy resolve
- `vegetation/vegetation.md`: vegetation과 surface feature placement plan
- `voxel/voxel.md`: column plan을 `ChunkData`로 채우는 graph-first voxel fill
- `created.md`: graph-first output을 bounded created-world dump로 저장하는 world-owned helper
- `preview/preview.md`: stage별 topdown preview binary 입력/출력 계약

빈 leaf 구현 파일은 만들지 않는다. 새 leaf를 실제로 구현할 때 같은 폴더에 대응 문서를 먼저 두고,
그 문서가 소유권과 불변식을 정의해야 한다.

---

## TDD / 검증 기준

새 generator는 구현보다 먼저 검증 표면을 가져야 한다.

### Determinism

- 같은 `(seed, generator_version, graph_region)`은 같은 graph patch를 만든다.
- 같은 `(seed, generator_version, world_x, world_z)`는 같은 column sample을 만든다.
- chunk별 생성과 area/stack batch 생성 결과가 일치한다.

### Seam

- 인접 chunk 경계 column이 일치한다.
- 인접 graph region 경계에서 site/corner/edge ownership이 안정적이다.
- padded graph patch를 다르게 요청해도 overlap 영역의 graph-derived sample이 같다.

### Artifact Suppression

- graph region 사각 경계가 height/material/biome에 보이지 않는다.
- raw Voronoi edge가 그대로 직선 river/coast/biome boundary로 보이지 않는다.
- noisy boundary가 edge guard 영역 밖으로 나가거나 이웃 edge와 교차하지 않는다.
- hard owner와 visible material boundary가 과도하게 같은 선을 따라가지 않는다.

### Hydrology

- river segment는 downstream progress를 가진다.
- flow accumulation은 합류 후 증가한다.
- lake terminal/inlet river는 raw flow ledger와 selected/display discharge를 구분하고, lake
  capacity에 따라 incoming chain 수, inlet marker raw-flow threshold, selected/display flow가
  제한되어야 한다. lake가 클수록 inlet threshold와 display cap은 함께 커지지만, ocean outlet trunk보다
  보수적인 상한을 유지해야 한다.
- lake contact river는 lake 내부 edge나 lake boundary edge를 selected segment로 사용하지 않고,
  land-side inlet/outlet endpoint marker에서만 lake와 만나야 한다. selected river segment 자체는
  lake corner를 endpoint로 삼지 않고, macro edge의 lake class도 `NonLake`여야 한다.
- 같은 selected river chain은 lake contact를 두 번 이상 가질 수 없다. lake inlet과 lake outlet은
  hydrology 연결 의미상 같은 lake system에 속할 수 있지만 selected chain은 inlet에서 종료되고 outlet에서
  새로 시작한다.
- selected river graph는 confluence/branch로 설명되지 않는 shared-corner intersection을 남기면 안 된다.
- preview-visible selected river graph는 valid confluence를 보존해야 한다. valid confluence는 같은
  vertex에 둘 이상의 selected incoming segment가 있고 정확히 하나의 selected outgoing segment가 이어지는
  형태다. outgoing 없는 terminal/non-outgoing corner에 모이는 여러 incoming은 명시 lake/sink/coast
  terminal로 설명되지 않으면 invalid intersection이다. 현재 river_plan은 합류 Q 재적분을 수행하지 않는다.
- selected river는 lake/sink/outlet 처리 없이 끊기지 않는다.
- local minima는 lake, sink, outlet carve 중 하나로 명시된다.
- river width는 flow와 안정적으로 연결된다.

### Macro Field / Heightfield

- macro field는 noise가 아니라 graph-derived signed distance / influence field cache다.
- macro field tile overlap은 인접 tile에서 같은 world-space sample에 대해 같은 값을 내야 한다.
- macro elevation, coast/lake/ocean/dry basin mask, ridge influence, river valley, combined macro
  height는 각각 finite 값과 문서화된 range를 유지해야 한다.
- pixelize/heightfield는 macro field contour preview와 같은 block-height domain을 사용해 column height를
  contour lower band로 resolve해야 하며, raw macro scalar를 버리고 contour line만 terrain source로
  재구성하면 안 된다. launch slice의 final land surface는 smoothing 없이 integer contour step을
  따른다.
- macro_field river valley는 selected river edge와 flow hint에서 만든 단순 guide다. 현실적인 width/depth
  continuity와 강 단면 정책은 heightfield/water 단계에서 다시 설계한다.
- ridge influence는 selected ridge path 주변에서 연결된 산맥 envelope를 진단할 수 있어야 하지만,
  broad mountain elevation model이 들어오기 전까지 combined macro height를 직접 올리지 않는다.
  ridge가 전역 low-level grain으로 퍼지거나 pinpoint maxima로 보이면 안 된다.
- selected hydrology endpoint, lake inlet/outlet, no lake-edge river invariant는 macro field
  rasterization 이후에도 유지되어야 한다.
- Perlin micro relief는 macro ownership, lake surface, river continuity를 뒤집으면 안 된다.
- 최종 heightfield preview는 흰색 texture에 단순 lighting을 적용한 top-down rendering으로 비어 있지
  않아야 하며, PNG metadata와 legend가 stage/channel을 명확히 기록해야 한다.

### Field Continuity

- 인접 site의 temperature/hydration/elevation bias는 비현실적으로 튀지 않는다.
- base graph field smoothing은 raw seed 대비 인접 site 차이를 줄여야 한다.
- base graph field의 `continentality`는 대륙성/해양성 site가 장거리로 뭉치는 coherent field여야 한다.
- base graph field의 `elevation_seed`는 macro elevation resolve가 읽을 수 있도록 continentality와
  완전히 독립된 salt-and-pepper noise가 아니어야 한다.
- corner base field와 elevation seed는 독립 random 값이 아니라 주변 site field에서 파생되어야 한다.
- biome transition은 gradient 또는 domain warp를 통해 완만하게 변한다.
- ocean/coast/lake/wetland 구분은 material policy와 topdown preview에서 일관된다.

---

## 의존성

허용:

- Rust standard library
- `world` 내부 public data contract
- deterministic geometry/noise helper

금지:

- `app`
- `ecs`
- `renderer`
- `platform`

`jobs`는 generation work를 실행할 수 있지만, generation 의미와 stage order를 소유하지 않는다.

---

## 불변식

1. graph region과 chunk boundary는 cache/output 단위일 뿐 visible terrain primitive가 아니다.
2. macro ownership과 macro elevation은 graph base field resolve로 먼저 생성되고, Perlin은 마지막 micro relief로만 합성된다.
3. 산맥/능선/단층 edge guide와 coast edge guide는 hydrology보다 먼저 정해진다.
4. hydrology는 최종 heightfield와 voxel fill 전에 selected river, valley, lake, coast 제약을 제공한다.
5. noisy boundary는 모든 Voronoi edge의 canonical geometry layer이며 raw graph topology를 대체하지 않는다.
6. meso feature는 macro ownership을 뒤집지 않고 macro_field가 bake할 deterministic feature plan을 제공한다.
7. polygon owner와 visible material/biome boundary는 분리될 수 있어야 한다.
8. material, water, vegetation은 직접 `ChunkData`를 수정하지 않고 plan으로 합쳐진 뒤 voxel fill에서 반영된다.
9. 각 stage는 topdown preview binary로 진단 가능해야 한다.
10. legacy generation re-export는 migration bridge이며, 새 graph-first 책임을 legacy 쪽으로 늘리지 않는다.
11. Delaunay/Voronoi graph construction과 macro ownership resolve는 chunk fill hot path에서 반복하지 않고,
    world-owned generation cache miss에서만 실행해야 한다.
12. macro field rasterization은 meso feature planning 이후, Perlin micro relief보다 먼저이며, chunk fill hot path는 graph query가
    아니라 macro field/pixelize/heightfield cache 샘플링을 수행해야 한다.
