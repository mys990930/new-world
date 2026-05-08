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
- edge 기반 hydrology, watershed, selected river chain의 생성 순서 정의
- graph region cache, macro map cache, hydrology/boundary/macro field/heightfield cache가 chunk fill hot path보다 먼저
  생성되고 공유되는 런타임 계약 정의
- noisy boundary, meso feature, continuous field, heightfield synthesis, surface plan, voxel fill 단계 경계 정의
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
   - signed macro elevation은 graph `elevation_seed`, `continentality`, coast distance, basinness를 합성해 만든다.
   - connected ocean coast에 인접한 land signed elevation은 해수면 `0` 근처에서 시작해야 한다.
     coast-adjacent site/corner는 낮은 양수 elevation을 갖고, graph coast distance가 커질수록 원래
     highland/mountain macro elevation을 회복한다. 이 coastal ramp는 높이 profile 전용이며, lake/wetland
     승격 정책을 과하게 넓히는 근거가 되어서는 안 된다.
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
     deterministic component roll을 함께 만족해야 하며, 모든 local minimum을 lake로 승격해서는 안 된다.
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
7. 모든 Voronoi edge를 canonical noisy boundary geometry로 현실화한다. raw graph topology는 그대로 보존한다.
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
8. graph guide, hydrology, noisy boundary를 합쳐 macro field tile을 rasterize한다.
   - 이 단계는 noise map 생성이 아니라 graph-derived signed distance / influence field cache 생성이다.
   - source of truth는 graph/macro/hydrology/boundary vector data에 남고, macro field는 heightfield와
     chunk sampler가 빠르게 읽기 위한 tile cache다.
   - 기본 channel은 macro elevation, coast/lake/ocean/dry basin mask, ridge/fault influence,
     river valley field, combined macro height다.
   - macro elevation은 graph signed elevation을 noisy boundary와 ownership context로 연속화한 값이다.
   - macro elevation과 ownership/mask 경계는 nearest-site 직선 경계가 아니라 stage 7
     `NoisyBoundaryCurve`의 side/blend 판정을 따라야 한다.
   - explicit coast edge는 일반 Voronoi boundary blend와 분리해 shoreline/foreshore profile로 샘플한다.
     canonical noisy coast curve 위와 아주 가까운 land-side sample은 `0`에 붙고, land 쪽으로 갈수록
     land owner elevation을 회복한다. ocean-side sample은 해수면 위로 섞이지 않고 얕은 음수/수중
     profile에서 ocean owner elevation으로 회복한다.
   - coast/lake/ocean/dry basin mask는 water surface, shoreline flatten, lake flatten, dry basin
     material policy가 읽는 distance/mask다.
   - ridge influence는 ridge edge가 산맥 local maxima guide라는 사실을 heightfield로 옮기기 위한
     distance-based envelope다. ridge 중심은 canonical noisy edge 위에 있고, 영향은 양옆으로 감쇠한다.
   - river valley field는 selected hydrology segment가 참조하는 canonical noisy edge를 anti-aliased
     thick polyline corridor로 구운 coverage/strength, nearest distance, flow를 저장한다. 강을 별도
     noise curve로 다시 만들지 않는다.
   - river valley field는 고정 폭으로 모든 강을 칠하지 않는다. selected/display flow가 작은 상류는
     좁고 얕은 carve guide를 만들고, flow가 큰 하류 trunk에서만 넓고 깊은 carve guide를 만든다.
   - river valley profile은 V자 center carve 하나가 아니라 flat-bottom + shoulder falloff 구조다.
     하류일수록 flat bed 폭이 커지고 side shoulder가 완만해지며, carve depth는 과도한 canyon을 피하도록
     capped된다.
   - river valley carve는 이 단계에서 2D scalar guide로 보이는 것이 정상이다. 최종 water surface와
     voxel carve는 heightfield/water/voxel 단계에서 확정하지만, combined macro height와 lit preview는
     이 guide가 지형을 낮추는 효과를 보여야 한다.
   - combined macro height는 아직 Perlin이 섞이지 않은 pre-micro 높이이며, 현재 launch slice에서는
     macro elevation, river carve, coast/lake flatten을 합성한다.
   - ridge guide와 ridge influence channel은 남아 있지만, narrow ridge envelope를 곧바로 높이에 더하면
     1블록 contour 기준에서 pinpoint maxima와 불연속적인 등고선 밀도 변화를 만들 수 있어
     `combined_macro_height`의 ridge raise는 broad mountain elevation model 재도입 전까지 disabled/stub으로 둔다.
   - dry basin은 lake/ocean water flatten 대상이 아니다. macro field에서는 폐쇄분지 surface mask와
     얕은 above-sea-level bowl profile로 표현한다. 내부 macro elevation variation과 dry/non-dry
     noisy boundary rim blend를 보존해 분지 내부에도 contour가 생겨야 하며, 큰 물웅덩이나 수면처럼
     낮추지 않는다.
   - 아직 micro Perlin이 없으므로 ordinary cell interior에 촘촘한 grain이 보이면 ridge/coast/river
     influence의 낮은 꼬리값이나 lit preview contrast가 과장된 것이다. ridge influence는 ridge guide
     주변에서만 active해야 하며 전역 low-level texture처럼 깔리면 안 된다.
9. meso feature plan을 만든다. 이 단계는 crater, ravine, dune field, hill cluster, terrace 같은 국소 지형 객체를 feature id와 world-space anchor로 배치한다.
10. seed 기반 Perlin micro relief를 만들고 hydrology/coast/lake/ridge/meso mask로 amplitude를 제한한다.
11. macro map, meso feature deformation, hydrology valley/lake/coast constraint, noisy boundary, Perlin micro relief를 합성해 heightfield와 water surface 후보를 만든다.
   - 현재 vertical slice에서는 meso feature와 Perlin micro relief를 stub으로 두고 각각 `0` delta를 적용한다.
   - `heightfield`는 stage 8 `MacroFieldTile`의 `combined_macro_height`와 mask/value channel을 column
     oriented `HeightfieldTile`로 변환한다.
   - heightfield는 `combined_macro_height`를 직접 continuous height로 쓰지 않고, stage 8 contour
     preview와 같은 block-height domain에서 contour lower band를 선택해 1-block integer terrace를
     만든다. 이 block-height domain은 signed sea level과 정렬되어 `combined_macro_height = 0`이
     `y = 0`이 되어야 한다. contour segment 자체는 debug layer이며 source of truth가 아니지만, column
     output은 같은 contour level domain과 일관되어야 한다.
   - launch macro heightfield는 실험적으로 큰 block-height domain을 사용한다. 관심 구간
     `combined_macro_height -0.25..0.75`는 `-512..1536 blocks`로 매핑하고, effective clamp는
     `-0.5..1.0 -> -1024..2048 blocks`다. 이 값은 `macro_field` contour와 `heightfield` band
     resolve가 공유한다.
   - launch contour terrace는 smoothing 없이 integer step을 유지하되, raw 1-block band를 그대로
     surface로 쓰지는 않고 `step + min_gap` stride로 visible terrace를 연다. 현재 기본 land
     `min_gap = 1`이고 river corridor 기본 `river_min_gap = 1`도 같은 값이다. 즉 raw height가
     2 block 진행될 때 visible terrain이 1 block 올라간다. river corridor override 구조는 남겨두어
     이후 water descent 보존이 다시 필요해지면 별도 gap으로 분리할 수 있지만, 현재 launch 기본값은
     land와 river가 같은 1-block minimum gap을 쓴다.
   - ocean/lake visible surface는 launch vertical slice에서 `y = 0`이다. bathymetry/bed depression은
     final preview terrain에 섞지 않고, standing water와 인접한 land는 `0, 1, 2, ...` contour step으로
     올라간다. river water hint도 integer step이며 인접 river/standing-water surface에서 큰 급락을
     만들지 않아야 한다.
   - 이 stage는 final block material이 아니라 surface height, water level, terrain kind hint를 제공하며,
     voxel fill은 이후 stage에서 별도로 수행한다.
   - `heightfield_preview`는 별도 수평 scale 계층을 쓰지 않는다. 같은 world footprint를 더 촘촘히
     보려면 해당 footprint를 더 많은 column으로 직접 샘플링한다. 현재 큰 block-height domain과
     cubic preview를 맞추기 위해 chunk-radius preview의 기본값은 chunk 하나당 32개 column, 즉
     기본적으로 1 world block당 1 column이며, free window preview는 기본 768개 X column을 사용한다.
     `--columns-x`/`--columns-z`가 지정되면 그것이 최종 column count다.
   - macro relief scale은 렌더링 트릭이 아니라 `macro_field` contour와 `heightfield` band resolve가
     공유하는 block-height domain에서 적용한다. 현재 실험 기본 signed scale은 effective
     `combined_macro_height -0.5..0.0..1.0 -> -1024..0..2048 blocks`이며, 중심 관심 구간
     `-0.25..0.75`는 `-512..1536 blocks`로 읽는다. preview는 이 산출 `surface_y`를 정육면체에 가까운
     block primitive로 그대로 렌더한다.
   - heightfield preview의 block outline은 기본 on이다. top/visible side face 외곽선과 side face의
     정수 `y` step guide를 얇게 그려 작은 chunk-radius preview에서 block scale을 읽게 하되, final
     mesh/material 계약으로 해석하지 않는다. 필요하면 preview 전용 `--no-block-lines`로 끌 수 있다.
12. elevation, water proximity, rain shadow, hydrology role을 반영해 final temperature/hydration/biome influence를 resolve한다.
13. biome/material/water/coast surface plan을 만든다.
14. vegetation/feature placement plan을 만든다.
15. heightfield, water, surface, vegetation plan을 한 번에 `ChunkData`로 voxel fill한다.

---

## 하위 모듈

현재 구현된 scaffold:

- `graph/graph.md`: graph region, Voronoi site/corner/edge id와 patch 계약
- `field/field.md`: hard owner가 아닌 continuous blended field sampling 계약
- `hydrology/hydrology.md`: watershed, drainage node, selected river edge 계약
- `pipeline/pipeline.md`: graph-first stage order와 column synthesis scaffold

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
  `continentality`를 land/ocean ownership의 source of truth로 읽고, graph adjacency component와
  coast distance를 통해 stage 3 ownership과 signed macro elevation을 resolve한다. 음수 water component도
  connectivity를 읽되 patch/open boundary 접촉만으로 ocean을 만들지 않고, explicit ocean basin
  component에 연결되지 않은 물은 inland lake candidate로 분류한다. stage 4 guide는
  같은 land component 내부성, signed elevation gradient, inlandness, mountainness/rugged context,
  drainage divide potential을 함께 읽어 ridge/fault edge candidate를 선택한다.
- stage 6 hydrology: macro guide와 graph topology를 읽어 selected river chain을 확정한다. 현재
  구현은 corner downhill, graph-stage local minimum, outlet carve, watershed, flow accumulation,
  selected river segment를 계산한다. 이 단계의 river는 후보 surface가 아니라
  downhill/local-minimum/outlet 정책을 통과한 결과다.
- stage 7 boundary: graph/macro annotation을 읽어 모든 Voronoi edge의 deterministic canonical noisy
  curve layer를 만든다. 구현은 Amit식 noisy edge 원칙을 따라 하나의 Voronoi edge의 두 corner와 두
  site center가 만드는 guard 안에서 midpoint displacement polyline을 생성한다. raw graph topology는
  그대로 남고, selected river는 별도 river curve가 아니라 hydrology segment의 edge id가 가리키는
  canonical curve를 따라 preview/heightfield에서 해석된다.
- stage 8 macro field: graph/macro/hydrology/boundary cache를 읽어 tile 단위 raster field를 만든다.
  이 field는 새 noise source가 아니라 heightfield와 chunk fill이 읽을 cache다. macro elevation,
  coast/lake/ocean/dry basin mask, ridge/fault influence, river valley, combined macro height는 각각
  독립 preview target이어야 하며, combined macro height는 Perlin 합성 전 결과만 표시한다. heightfield
  직전 macro field 연속성을 진단하기 위해 block-height 기준 contour preview를 추가로 뽑을 수 있어야 한다.
- stage 11 heightfield: 현재 구현은 `MacroFieldTile`을 읽어 `HeightfieldTile` column cache로 변환한다.
  meso/perlin delta는 아직 `0`인 stub이며, macro field contour step과 일관된 band interpolation을
  거친 뒤 integer block height로 snap한다. ocean/lake mask는 water level hint로, river/ridge/dry basin
  channel은 terrain kind hint로 보존한다.

런타임에서는 위 stage를 chunk마다 반복 실행하지 않는다. `pipeline/pipeline.md`의 runtime cache
contract에 따라 graph region cache, macro map cache, hydrology/boundary cache, macro field tile cache,
micro relief/heightfield cache를 worker에서 준비하고, chunk generation은 필요한 world-space
column/window만 sample해 `ChunkData`를 채운다.

문서화된 다음 leaf:

- `boundary/boundary.md`: 모든 Voronoi edge의 canonical noisy geometry 계약
- `meso_feature/meso_feature.md`: 국소 지형 feature planning과 heightfield deformation 계약
- `macro_field/macro_field.md`: graph-derived signed distance / influence field tile cache
- `heightfield/heightfield.md`: macro field와 Perlin micro relief 합성
- `surface_plan/surface_plan.md`: biome, material, water/coast/wetland policy resolve
- `vegetation/vegetation.md`: vegetation과 surface feature placement plan
- `voxel/voxel.md`: column plan을 `ChunkData`로 채우는 graph-first voxel fill
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
- preview-visible selected river graph는 같은 corner에 여러 독립 incoming chain이 겹쳐 보이지 않도록
  occupancy/merge 정책을 적용해야 한다. 명시 confluence geometry가 생기기 전까지는 가장 큰 selected
  incoming branch만 남기고 나머지 upstream selected tree를 제거한다.
- selected river는 lake/sink/outlet 처리 없이 끊기지 않는다.
- local minima는 lake, sink, outlet carve 중 하나로 명시된다.
- river width는 flow와 안정적으로 연결된다.

### Macro Field / Heightfield

- macro field는 noise가 아니라 graph-derived signed distance / influence field cache다.
- macro field tile overlap은 인접 tile에서 같은 world-space sample에 대해 같은 값을 내야 한다.
- macro elevation, coast/lake/ocean/dry basin mask, ridge influence, river valley, combined macro
  height는 각각 finite 값과 문서화된 range를 유지해야 한다.
- heightfield는 macro field contour preview와 같은 block-height domain을 사용해 column height를
  contour lower band로 resolve해야 하며, raw macro scalar를 버리고 contour line만 terrain source로
  재구성하면 안 된다. launch slice의 final land surface는 smoothing 없이 integer contour step을
  따른다.
- river valley width와 depth는 selected/display flow에 단조 증가해야 한다. 상류와 하류가 같은 폭으로
  보이면 회귀다.
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
6. meso feature는 macro ownership을 뒤집지 않고 heightfield가 읽을 deterministic deformation plan을 제공한다.
7. polygon owner와 visible material/biome boundary는 분리될 수 있어야 한다.
8. material, water, vegetation은 직접 `ChunkData`를 수정하지 않고 plan으로 합쳐진 뒤 voxel fill에서 반영된다.
9. 각 stage는 topdown preview binary로 진단 가능해야 한다.
10. legacy generation re-export는 migration bridge이며, 새 graph-first 책임을 legacy 쪽으로 늘리지 않는다.
11. Delaunay/Voronoi graph construction과 macro ownership resolve는 chunk fill hot path에서 반복하지 않고,
    world-owned generation cache miss에서만 실행해야 한다.
12. macro field rasterization은 Perlin micro relief보다 먼저이며, chunk fill hot path는 graph query가
    아니라 macro field/heightfield cache 샘플링을 수행해야 한다.
