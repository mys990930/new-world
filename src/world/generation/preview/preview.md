# preview

## 역할

`preview`는 generation stage별 topdown preview binary의 입력/출력 계약을 소유한다.

초기 generator에서는 기능 완성도만큼 진단 가능성이 중요하다. 각 stage는 chunk 생성 없이도 같은
입력으로 같은 preview를 만들 수 있어야 한다.

---

## 책임

- stage별 preview input area, seed, generator version, config 계약
- deterministic PNG 또는 raw dump 출력 계약
- graph, macro map, hydrology, river plan, boundary, field, macro field, pixelize, heightfield, surface 결과를 독립적으로 검사할 수 있는 surface 정의
- artifact regression을 테스트와 연결할 수 있는 기준 제공

---

## 비책임

- GPU rendering
- interactive editor UI
- runtime chunk storage mutation
- generation stage 의미 소유

---

## Preview Data Cache

- preview binary는 기본적으로 `target/preview-cache/<binary>/` 아래 bincode data cache를 먼저 읽는다.
- `--refresh`가 있을 때만 해당 preview 데이터를 다시 계산하고 cache를 갱신한다.
- cache key는 preview 입력 인자, generator version, 그리고 data-producing source file fingerprint를
  포함한다. 따라서 stage 산출 데이터 로직이 바뀌면 기본 실행도 낡은 cache를 재사용하지 않고 새로
  계산한다.
- graph-first preview는 PNG 결과물이 아니라 graph/macro/hydrology/boundary/macro-field/pixelize/heightfield
  계열 stage 산출 데이터를 cache한다. PNG metadata와 overlay는 cache된 data에서 다시 렌더된다.
- `macro_field_preview`의 base cache는 `PreviewWorld`와 world-owned `MacroFieldTile`까지 저장한다.
  contour step, major interval, channel, overlay 여부, output path는 render 파생 옵션이므로 cache key에
  들어가지 않는다. 따라서 contour 조건만 바꾸면 저장된 macro field tile에서 contour만 다시 추출해
  빠르게 PNG를 다시 뽑는다.
- legacy renderer preview는 아직 world generation stage cache 타입으로 분리되지 않은 부분이 있어,
  PNG 파일 자체가 아니라 PNG 인코딩 전 raw preview payload를 cache한다.

---

## Preview 대상

초기 preview 우선순위:

1. site/corner/edge graph preview
2. graph region ownership preview
3. graph-field-resolved continent/ocean/island ownership preview
4. graph-field-resolved macro elevation preview
5. ridge/fault edge and coast edge preview
6. dominant site map
7. blended influence map
8. elevation/corner downhill arrow map
9. watershed map
10. river flow accumulation map
11. river reach morphology plan preview
12. final temperature/hydration/biome influence preview
13. noisy edge preview
14. macro field rasterization preview
15. chunk pixelize preview
16. heightfield / voxel-column realization preview
17. biome/material/surface plan preview
18. vegetation placement preview
19. voxel fill preview

`generation_preview_suite`는 새 stage를 만들지 않고, 위 preview 대상 중 현재 핵심 graph-first 흐름을
한 폴더에 모으는 orchestration binary다. suite output은 stage별 source of truth가 아니며, 각 PNG의
의미와 metadata 계약은 호출된 child preview binary 문서가 소유한다.

---

## `graph_voronoi_preview` CLI 계약

`graph_voronoi_preview`는 graph-first pipeline의 첫 단계인 Voronoi macro graph를 chunk 생성 없이
검사하는 topdown preview binary다.

### 입력

- 필수 positional 인자: `<seed> <center-x> <center-z>`
  - `center-x`, `center-z`는 world-block 좌표다.
- 선택 인자:
  - `--width <u32>`: 기본 `3840`
  - `--height <u32>`: 기본 `2160`
  - `--world-span-blocks <i32>`: 이미지 가로가 덮는 world-block 폭, 기본 `32768`
  - `--region-size-blocks <i32>`: graph cache region 크기, 기본 `DEFAULT_GRAPH_REGION_SIZE_BLOCKS`
  - `--site-spacing-blocks <i32>`: preview site 간격, 기본 `DEFAULT_SITE_SPACING_BLOCKS`
  - `--stage graph_voronoi`
  - `--mode <all|identity|temperature|hydration|humidity|continentality|elevation|ruggedness>`: 기본 `identity`
  - `--output <path>`

### 출력

- 기본 출력은 `target/graph-voronoi-preview/` 아래 PNG다.
- 기본 단일 mode 출력 파일명은 `s<seed>_x<center-x>_z<center-z>_<mode>.png`처럼 seed, center,
  mode만 담는다.
- `--mode all`은 `identity`, `temperature`, `hydration`, `continentality`, `elevation`, `ruggedness`
  PNG를 `s<seed>_x<center-x>_z<center-z>/` 같은 짧은 이름의 디렉터리에 생성한다. 이때 `--output`은
  디렉터리 경로여야 한다.
- 단일 mode에서 `--output`이 확장자를 가진 경로이면 기존처럼 해당 PNG 파일에 쓴다. 확장자가 없는 경로이면
  디렉터리로 보고 `<mode>.png` 파일을 그 아래에 쓴다.
- width, height, generator version, stage, world span, site spacing은 파일명에 넣지 않고 PNG metadata에만
  기록한다.
- PNG에는 `new-world-preview-header` iTXt metadata chunk가 들어가며 graph area, site count,
  owner region count, mode/map name, nearest-site spacing min/avg/max/stddev/CV를 함께 기록한다.
- 각 PNG는 작은 legend overlay를 가진다. field map은 gradient color bar와 양끝 의미 label을 표시하고,
  identity map은 간단한 header만 표시한다.
- 각 PNG는 방향 compass overlay를 가진다. macro field/world topdown 기준으로 이미지 위쪽은 북(N,
  `world -Z`), 오른쪽은 동(E, `world +X`), 아래쪽은 남(S), 왼쪽은 서(W)를 뜻한다.
- 픽셀 생성은 Rayon 병렬 chunk 처리로 수행한다.

### 현재 구현 상태

- `graph_voronoi_preview`는 `world::generation::graph::generate_voronoi_graph_patch(...)`를 호출해
  같은 graph contract를 시각화한다.
- binary는 graph 의미를 새로 만들지 않고, preview window에서 필요한 padding을 계산한 뒤
  `VoronoiGraphPatchRequest`를 구성한다.
- identity mode의 fill layer는 픽셀마다 nearest site를 칠하는 진단용 raster다. 이 raster의 경계는
  “가장 가까운 site가 바뀌는 위치”를 보여주며, 실제 공개 graph edge를 재구성한 것이 아니다.
- 실제 topology overlay는 `VoronoiEdge.corners`가 참조하는 `VoronoiCorner.position` 두 점을
  world-space에서 clipping/projection한 corner-to-corner segment로 그린다. 따라서 nearest-site raster
  경계와 Delaunay/circumcenter Voronoi dual edge overlay는 일부 위치에서 다르게 보일 수 있다.
- stdout과 PNG metadata는 nearest-site distance 통계를 포함한다. 이 값은 graph cell 크기가 지나치게
  균일한 격자로 돌아가거나, 반대로 극단적으로 작은 cell이 생기는 문제를 빠르게 확인하기 위한
  진단값이다.
- temperature, hydration/humidity, continentality, elevation map은 graph base-field stage가 만든
  smoothed `VoronoiSite::base_fields`를 색상 gradient로 표현한다.
- ruggedness map은 아직 smoothing 대상이 아닌 site-level roughness seed를 표현한다.
- 픽셀 sampling과 PNG encoding은 preview binary 책임이며, graph 생성 정책은 `graph` 모듈이
  소유한다.

---

## `macro_map_preview` CLI 계약

`macro_map_preview`는 graph-first pipeline의 graph-field-resolved macro map stage를 chunk 생성 없이
검사하는 topdown composite preview binary다.

### 입력

- 필수 positional 인자: `<seed> <center-x> <center-z>`
  - `center-x`, `center-z`는 world-block 좌표다.
- 선택 인자:
  - `--width <u32>`: 기본 `3840`
  - `--height <u32>`: 기본 `2160`
  - `--world-span-blocks <i32>`: 이미지 가로가 덮는 world-block 폭, 기본 `32768`
  - `--region-size-blocks <i32>`: graph cache region 크기, 기본 `DEFAULT_GRAPH_REGION_SIZE_BLOCKS`
  - `--site-spacing-blocks <i32>`: preview site 간격, 기본 `DEFAULT_SITE_SPACING_BLOCKS`
  - `--land-bias <f32>`: `MacroMapConfig.land_bias`, 양수는 land ownership을 늘리고 음수는 ocean ownership을 늘림
  - `--stage macro_map`
  - `--output <path>`

### 출력

- 기본 출력은 `target/macro-map-preview/` 아래 PNG다.
- 기본 출력 파일명은 `s<seed>_x<center-x>_z<center-z>.png`처럼 seed와 center만 담는다.
- 단일 PNG에서 ocean/lake는 파란색, coast는 sandy color, 내륙은 초록 계열, 고지대는 회백색,
  가장 높은 peak는 흰색으로 표현한다.
- 모든 graph Voronoi edge는 실제 graph corner-to-corner segment를 기준으로 희미한 base overlay로
  표시한다.
- stage 9 boundary overlay는 모든 graph Voronoi edge의 canonical noisy curve를 그린다. 이 overlay는
  straight edge를 재샘플링한 선이 아니라 endpoint는 유지하고 중간 point를 edge normal 방향으로
  흔든 울퉁불퉁한 polyline이어야 한다. boundary curve는 독립 jitter가 아니라 correlated wave/value
  noise와 smoothing을 거친 자연스러운 곡선이어야 하며, 기본 4K preview와 640 smoke preview에서도
  식별 가능한 amplitude를 가져야 한다.
- ridge candidate edge는 흰색, fault candidate edge는 붉은색, coast candidate edge는 sandy color overlay로 표시한다.
- macro_map preview는 macro_map fill과 stage 4/5 guide를 기본 layer로 표시하고, stage 6 hydrology가
  구현된 뒤에는 같은 composite 위에 selected river result를 추가 overlay한다.
- selected river는 hydrology가 확정한 `GraphRiverSegment`만 표시한다. pre-hydrology river candidate나
  macro_map river potential은 표시하지 않는다.
- selected river segment는 stage 9 `BoundaryCache`가 제공하는 canonical noisy edge geometry를 따라 cyan/blue line으로 표시하고,
  segment의 selected/display `flow_accumulation`이 클수록 더 두껍게 그린다. hydrology raw corner
  accumulation과 lake capacity가 적용된 selected discharge가 다를 수 있으며, lake terminal/inlet
  river는 ocean outlet river보다 보수적인 두께 cap을 가진다. lake terminal/inlet은 hydrology
  단계에서 chain 수와 inlet marker raw-flow threshold가 제한되지만, 기준을 통과한 lake-bound trunk는
  lake boundary 직전 land-side endpoint까지 표시될 수 있다.
- lake fill은 파란색 계열로 표시한다. `CoastOcean`처럼 ocean-owned coast transition에 속하는 영역도
  사용자 눈에는 바다로 읽히도록 파란 계열을 유지한다. sandy/pale yellow 계열은 land-side coast
  overlay나 coast land fill에만 사용한다.
- dry basin fill은 지속 수면이 아닌 폐쇄 저지대로 읽히도록 muted olive/khaki 계열로 표시한다.
- sink/outlet node는 작은 marker로 표시한다. internal lake debug node는 기본 preview에서 표시하지 않는다.
- lake inlet과 lake outlet node가 있으면 서로 다른 marker 색과 짧은 방향 화살표로 표시한다.
  이 marker는 hydrology selected graph endpoint marker다. inlet marker는 lake boundary 직전의
  land-side endpoint에 incoming selected segment가 있을 때만 생기고, outlet marker는 lake boundary
  바깥의 land-side endpoint에 outgoing selected segment가 있을 때만 생긴다. river segment 자체는
  lake boundary edge 위에 그려지지 않는다. marker와 화살표는 river line과 node dot 위에서도 보이도록
  hydrology overlay의 마지막 쪽에서 그린다. inlet/outlet 화살표는 selected segment 전체 길이를 덮는
  긴 shaft가 아니라 endpoint 근처의 짧은 방향 표시여야 하며, lake boundary edge 위에 선처럼 놓이면 안 된다.
- selected river, `LakeInlet`, `LakeOutlet`, `CoastOutlet`, lake fill 색은 서로 구분되어야 한다.
  `GraphDrainageNodeKind::Lake`는 local minimum lake resolution을 나타내는 내부 debug node이며,
  기본 macro map preview legend와 overlay에는 표시하지 않는다.
- metadata/stdout에는 lake inlet count, lake outlet count, invalid lake contact count,
  invalid river intersection count, ambiguous shared corner count, duplicate trunk pruned count,
  repeated lake contact pruned count, unclassified lake-connected flow count,
  disconnected lake inlet/outlet count, selected lake-edge river segment count, visible site count,
  land site count, land ratio, small lake component count를 포함한다. 정상 preview에서
  disconnected, selected lake-edge, invalid, ambiguous count는 0이어야 하며, duplicate trunk pruned count는
  selected overlay에서 제거한 중복 upstream branch 수를 나타낸다. repeated lake contact pruned count는
  같은 selected chain이 두 번째 lake contact에 닿지 않도록 제거한 segment 수를 나타낸다.
- 작은 legend overlay는 ocean/lake/land/dry fill, ridge/fault/coast edge, selected river, sink,
  inlet/outlet marker key를 포함한다. 숨겨진 debug-only lake node는 legend에 넣지 않는다.
- 방향 compass overlay는 legend와 겹치지 않는 위치에 표시하며, 이미지 위=N(`world -Z`),
  오른쪽=E(`world +X`), 아래=S, 왼쪽=W의 macro field 기준을 따른다.
- width, height, generator version, stage, world span, site spacing은 파일명에 넣지 않고 PNG
  metadata에만 기록한다.
- PNG에는 `new-world-preview-header` iTXt metadata chunk가 들어가며 graph area, site count,
  candidate edge count, coast/ridge/fault edge count, selected river segment count, lake/sink/outlet
  node count, visible site count, land site count, land ratio, lake component count,
  small lake component count, inland water site count, ocean component count,
  lake/ocean terminal river segment count, lake-capped segment count, lake inlet/outlet count,
  disconnected lake inlet/outlet count, selected lake-edge river segment count,
  invalid lake contact/intersection count, ambiguous shared corner count, duplicate trunk pruned
  count, unclassified lake-connected flow count, dry basin site count, max lake component size,
  large lake component count, lake/ocean max display/raw flow, lake inlet raw/display flow range,
  boundary curve count, boundary profile counts, boundary guard violation count,
  boundary average/max amplitude blocks, boundary average/max pixel displacement,
  boundary nearly-straight curve count,
  sea level, stage 4 guide input note, stage 6 hydrology raw/selected flow note, source note를 함께 기록한다.
- 단일 preview에서 `--output`이 확장자를 가진 경로이면 해당 PNG 파일에 쓴다. 확장자가 없는 경로이면
  디렉터리로 보고 기본 짧은 파일명을 그 아래에 쓴다.
- 픽셀 생성은 Rayon 병렬 chunk 처리로 수행한다.

### 현재 구현 상태

- `macro_map_preview`는 `world::generation::graph::generate_voronoi_graph_patch(...)`로 graph patch를
  만들고, `world::generation::generate_macro_map(&patch, MacroMapConfig::new(...))`로 macro map을
  생성한다.
- 목표 계약상 continent/ocean/island ownership과 signed elevation은 graph base `continentality`와
  `elevation_seed`를 `macro_map`이 resolve한 결과다. 현재 구현도 macro_map 내부 super-cell
  continent/island field 없이 graph base field를 source of truth로 사용한다.
- lake/coast, ridge/fault 후보 의미는 `macro_map` 모듈이 소유한다. selected river 의미는 hydrology
  모듈이 소유한다.
- fill layer는 픽셀마다 nearest site의 macro annotation을 칠하는 진단용 색상면이다. 따라서
  nearest-site color boundary는 실제 graph edge overlay와 정확히 같은 선처럼 보이지 않을 수 있다.
  overlay는 nearest-site 경계에서 재구성하지 않고 `MacroEdge.corners`와 graph patch의
  `VoronoiCorner.position`이 정의한 실제 corner segment를 world-space clip/projection해서 그린다.
- coast/ridge/fault candidate overlay는 nearest-site fill 근사 경계에 맞추지 않고,
  `MacroEdge.corners`가 참조하는 graph patch의 실제 `VoronoiCorner.position` 두 점을 world-space에서
  clipping한 뒤 픽셀 중심 좌표계로 투영한 선분으로 그린다.
- hydrology overlay는 stage 9 이후 같은 edge id의 canonical noisy curve를 사용한다. 따라서 selected river는 raw
  nearest-site raster boundary가 아니라 hydrology가 선택한 Voronoi edge chain의 noisy geometry 위에 놓인다.
- hydrology overlay는 macro_map의 명시적 lake edge class를 통과한 selected segment만 그린다. 따라서
  nearest-site lake fill 경계와 corner surface 기준이 어긋나도 selected river가 lake boundary/internal/
  adjacent edge 위에 그려지면 회귀다.
- 픽셀 sampling과 PNG encoding은 preview binary 책임이다.

---

## `macro_field_preview` CLI 계약

`macro_field_preview`는 stage 10 macro field rasterization을 chunk 생성 없이 검사하는 topdown preview
binary다.

macro field는 noise source가 아니다. 이 preview는 graph/macro/hydrology/river-plan/final-cell-context/boundary cache를
world-space sample grid로 굽는 과정을 검사한다. source of truth는 graph topology, macro annotation,
selected hydrology result, river reach morphology plan, canonical noisy boundary에 남고, `MacroFieldTile`은 pixelize와 downstream heightfield/chunk fill이
빠르게 읽기 위한 graph-derived signed distance / influence field cache다.

preview와 runtime cache miss는 ridge/coast/river guide distance를 sample마다 반복 계산하지 않아야
한다. stage 10 preview는 ridge/coast의 canonical noisy curve를 tile source pixel로 rasterize하고
distance propagation으로 influence field를 만들 수 있다. river channel은 stage 7 `RiverPlan`의
broad valley parameter와 narrow bed hint를 같은 noisy edge geometry 위에 굽는다. 이 pass는
selected hydrology topology를 바꾸지 않고, subpixel coverage 기반 valley strength, nearest guide
distance, blended display flow, reach type을 저장한 뒤 그 결과를 렌더한다. river rasterization은
row-range local buffer를 Rayon worker가 독립적으로 채우고 row-major 순서로 결합해 deterministic
multi-thread output을 유지한다.
ownership/mask의 noisy-boundary side 판정은 정확도 유지를 위해 launch 단계에서 per-sample query가
남을 수 있지만, 이 비용은 chunk fill hot path가 아니라 macro field tile cache miss에 한정된다.
sample fill은 site bucket 후보를 allocation 없이 직접 순회하고, nearest polyline 후보는 제곱거리로
비교한 뒤 최종 selected distance만 계산해 deterministic output을 유지하면서 CPU scalar work를 줄인다.

### 입력

- 필수 positional 인자: `<seed> <center-x> <center-z>`
  - `center-x`, `center-z`는 world-block 좌표다.
- 선택 인자:
  - `--width <u32>`: 기본 `3840`
  - `--height <u32>`: 기본 `2160`
  - `--world-span-blocks <i32>`: 이미지 가로가 덮는 world-block 폭, 기본 `32768`
  - `--chunk-radius <i32>`: world-block center를 유지한 채 가로 footprint를 `radius * 2 * CHUNK_EDGE`
    block으로 정한다. 기본 footprint `32768`은 현재 `CHUNK_EDGE = 32` 기준 `chunk_radius = 512`와 같다.
    heightfield density의 작은 window가 필요하면 `--world-span-blocks 1024 --width 1024` 또는 작은
    `--chunk-radius` zoom-in을 명시한다.
  - `--stage macro_field`
  - `--channel <all|macro|mask|ridge|river|combined|lit|contour>`: 기본 `lit`
  - `--contour-step <blocks>`: contour channel과 overlay가 사용할 block-height 간격, 기본 `32`
  - `--contour-major-every <n>`: major contour 간격 multiplier, 기본 `5`
  - `--contours`: `combined`/`lit` channel 위에 contour overlay를 추가
  - `--output <path>`

### Preview Checklist

각 channel은 독립 PNG로 뽑을 수 있어야 하며, `--channel all`은 아래 항목을 모두 생성한다.

- `macro`: graph signed macro elevation을 noisy boundary/ownership context로 연속화한 field
- `mask`: ocean, lake, dry basin, explicit ocean coast, land mask와 distance band. 경계는 straight nearest-site raster가
  아니라 `BoundaryCache`의 canonical noisy curve를 따라 보여야 한다.
  coast key는 connected ocean basin과 non-ocean terrain 사이의 shoreline만 의미하며, dry basin/lake와
  land 사이의 경계가 노란 coast처럼 보이면 회귀다.
- `ridge`: ridge/fault guide edge의 canonical noisy curve 주변 influence envelope
- `river`: selected hydrology segment가 참조하는 canonical noisy curve 주변의 river plan broad
  valley strength, narrow bed hint, display flow, reach type. 이 channel은 모든 강을 같은 폭으로
  칠하지 않고, 상류/하류와 lake inlet/outlet의 morphology 차이를 보여야 한다. macro field combined
  height는 broad valley를 주로 반영하고, narrow bed는 heightfield/water가 읽을 hint로 보존한다.
- `combined`: Perlin 합성 전 macro elevation + ridge raise - broad river valley - lake flatten 결과.
  이 단계의 river effect는 최종 water/voxel carve가 아니라 heightfield가 읽을 2D broad valley guide이며,
  combined/lit preview에서 좁은 물길을 과하게 새기면 회귀다. `combined`는 진단용 heat map이 아니라
  macro base 위에 ridge와 broad valley가 얹힌 pre-Perlin terrain surface로 읽히도록 subtle terrain
  ramp를 사용한다.
- `lit`: combined macro height 또는 heightfield stage output을 흰색 texture와 단순 lighting으로
  보여주는 top-down rendering
- `contour`: heightfield 직전 `combined_macro_height`를 block-height scale으로 변환한 뒤 Marching
  Squares로 추출한 contour line preview. 기본 level step은 32 blocks이며, 5 level마다 major contour를
  그린다. sea level `y=0` contour는 별도 blue 계열로 표시한다.
- `cell_context` 또는 `biome`: stage 8에서 resolve된 final temperature, hydration, hydrology role,
  water proximity, rain shadow, biome influence를 표시한다. 이 preview는 macro_field가 biome을
  새로 분류하는 표면이 아니라, pre-macro_field final cell context를 tile/sample 형태로 보존했는지
  확인하는 표면이다.

중간 단계는 2D gradient/mask preview여야 한다. 최종 산출물은 색상 지형도가 아니라 흰색 texture에
간단한 normal/light shading을 입힌 top-down heightfield rendering이어야 한다. lighting은 진단용이며
renderer/GPU 계약을 만들지 않는다.

### 출력

- 기본 출력은 `target/macro-field-preview/` 아래 PNG다.
- 기본 파일명은 `s<seed>_x<center-x>_z<center-z>_<channel>.png`처럼 짧게 유지한다.
- width, height, generator version, stage, world span, tile resolution은 파일명에 넣지 않고 PNG
  metadata에만 기록한다.
- PNG에는 `new-world-preview-header` iTXt metadata chunk가 들어간다.
- metadata/stdout은 tile bounds, sample resolution, source graph/macro/hydrology/river-plan/final-cell-context/boundary version,
  channel name, min/max/avg, absolute preview scale, robust percentile diagnostic range, noisy
  boundary displacement stats, finite/NaN count, overlap guard width, source cache key, legend
  labels, influence source curve/pixel count, tile generation timing을 기록한다.
- 각 PNG는 작은 legend overlay를 가진다. gradient channel은 color bar와 low/high 의미를 표시하고,
  mask channel은 ocean/lake/dry/coast/land key를 서로 구분되는 색으로 표시한다. lit heightfield는 height range와 light
  direction만 표시한다. contour channel은 minor/major/sea-level key와 contour step/major spacing을 표시한다.
- 각 PNG는 별도 방향 compass overlay를 포함한다. 방향 기준은 모든 topdown macro field preview와
  같아서 위=N, 오른쪽=E, 아래=S, 왼쪽=W다.
- 모든 `macro_field_preview` channel은 stage 9 `BoundaryCache`의 canonical noisy Voronoi graph edge
  overlay를 표시한다. 이 overlay가 사용자가 요청한 terrain tile/boundary 확인의 기본 표면이지만,
  field 값을 압도하면 안 된다. 기본 스타일은 위치 참고용 faint overlay이며, 색과 opacity는
  macro/combined/lit 값을 먼저 읽을 수 있을 정도로 약해야 한다.
- `macro_field_preview`는 hydrology가 선택하고 `RiverPlan`이 morphology를 부여한 모든 river segment의
  centerline을 같은 `BoundaryCache` canonical noisy curve 위에 별도 cyan/blue overlay로 표시한다.
  본류성 reach(`Lower`, `Trunk`, `LakeOutlet`)는 지류보다 약간 더 두껍고 밝게 그리되, 이는 강폭
  rasterization 결과가 아니라 강 중심 줄기를 확인하기 위한 reference line이다. 상류/지류와 본류가
  모두 표시되어야 하며, metadata/stdout은 selected centerline segment 수, 화면에 그려진 clipped
  polyline segment 수, 본류/지류 segment 수를 기록한다.
- `macro_field_preview`는 river centerline 위에 selected source marker를 기본으로 표시한다.
  outgoing selected segment를 가진 `GraphDrainageNodeKind::Source` node가 marker 대상이며, downstream
  selected path가 confluence에 먼저 닿으면 tributary source amber/yellow ring, coast/lake terminal에
  먼저 닿으면 mainstem source bright cyan/white ring으로 그린다. marker는 legend, scale bar, compass
  아래 layer에 있어야 하며 metadata/stdout은 mainstem source marker 수, tributary source marker 수,
  실제 viewport 안에 그려진 marker 수를 기록한다.
- `lit` channel은 broad hillshade가 우선 읽혀야 하므로 다른 channel보다 더 희미한 Voronoi edge
  overlay를 사용한다. lit에서 edge가 조명/고저차보다 먼저 보이면 회귀다.
- `macro`, `combined`, `lit` channel은 sampled macro field에서 ocean/lake water와 terrain이 맞닿는
  곳에 더 두꺼운 standing-water boundary overlay를 그린다. Water 판정은 ocean mask 또는 lake mask이고,
  dry basin과 explicit ocean coast는 terrain으로 취급한다. 이 overlay는 희미한 Voronoi reference edge보다
  진한 노란색으로 더 쉽게 읽혀야 하지만 channel 값을 완전히 압도하면 회귀다.
- macro-field cache tile grid는 보조 진단 overlay로 유지할 수 있지만, graph edge overlay보다 강하게
  읽히면 안 된다. 이 grid는 각 tile 내부에서 height를 따로 low/high normalize한다는 뜻이 아니다.
- 모든 `macro_field_preview` output은 world footprint를 이해할 수 있도록 scale bar를 표시한다.
- `--contours`가 지정되면 `combined`와 `lit` channel에 contour overlay를 추가할 수 있다. 이 overlay는
  preview 진단용이며 macro field 값 자체를 바꾸지 않는다.
- `lit` channel은 preview lighting artifact를 줄이기 위해 combined height 데이터를 변경하지 않고
  lighting normal 계산에만 smoothing/prefilter를 적용할 수 있다. 이때 목표는 tile 하나하나의
  sample-level lighting이 아니라, 전체 지형 고저차를 흰색 재질의 broad hillshade로 읽는 것이다.
  metadata/stdout은 raw gradient, smoothed-normal gradient, broad hillshade brightness range/stddev를
  기록해 실제 combined height 변화와 lighting-only smoothing/contrast를 구분해야 한다.
- 픽셀 생성은 Rayon 병렬 chunk 처리로 수행한다.

기본 `macro_field_preview`는 stage 비교를 위해 `macro_map_preview`와 같은 `32768` world-block
overview footprint와 `3840 x 2160` sample grid를 사용한다. `MacroFieldTile` 타입 자체는 여전히
`width * sample_spacing_blocks` by `height * sample_spacing_blocks` footprint를 갖는 일반 raster
surface이며, runtime cache와 pixelize handoff의 1-block density는 `1024` block tile 또는 명시적
zoom-in preview에서 확인한다.

### 검증 기준

- determinism: 같은 seed/config/tile/channel은 같은 PNG와 metadata를 만든다.
- adjacent tile overlap stability: 인접 tile overlap의 같은 world-space sample은 같은 값을 가진다.
- preview scale continuity: `macro`, `combined`, `lit` channel은 preview image나 tile마다 local
  min/max를 다시 잡지 않고 문서화된 absolute normalized scale로 렌더해야 한다.
- finite/range sanity: 모든 channel은 finite 값이며 문서화된 range를 벗어나지 않는다.
- hydrology endpoint attachment: river valley field는 selected hydrology segment와 `RiverPlan`이
  정의한 reach parameter, lake inlet/outlet endpoint를 따라가야 한다.
- noisy boundary ownership: macro elevation과 mask 경계는 nearest-site straight boundary가 아니라
  stage 9 noisy boundary curve를 따라야 한다.
- no lake-edge river invariant: river valley/bed hint는 `MacroLakeEdgeClass::NonLake` selected
  segment와 lake endpoint marker 정책만 사용해야 한다.
- preview nonblank: 각 channel은 blank 단색 이미지가 아니어야 하며 legend와 metadata를 포함해야 한다.
- contour sanity: contour segment는 finite world-space endpoint를 가져야 하며, flat field는 contour를
  만들지 않고 simple ramp field는 crossing contour를 만들어야 한다.

---

## `pixelize_preview` CLI 계약

`pixelize_preview`는 stage 11 chunk pixelize output을 chunk 생성 없이 검사하는 topdown preview
binary다.

이 preview는 stage 10 `MacroFieldTile`을 chunk-aligned `PixelizedChunkArea`로 변환한 뒤, 각 resolved
column을 하나의 PNG pixel로 그린다. source of truth는 `MacroFieldTile.samples[]`에 들어 있는
graph-derived field이며, preview renderer가 graph topology, hydrology, river plan, noisy boundary를
직접 다시 query하면 안 된다.

### 입력

- 필수 positional 인자: `<seed> <cx> <cz> <r>`
  - `cx`, `cz`는 chunk coordinate다.
  - `r`은 inclusive square chunk radius이며, footprint는 `(2r + 1)` chunks on each horizontal axis다.
- 선택 인자:
  - `--width <u32>`: 출력 PNG width. 기본은 sampled column count와 일치한다.
  - `--height <u32>`: 출력 PNG height. 기본은 sampled column count와 일치한다.
  - `--region-size-blocks <i32>`
  - `--site-spacing-blocks <i32>`
  - `--land-bias <f32>`
  - `--stage pixelize`
  - `--output <path>`

### 출력

- 기본 출력은 `target/pixelize-preview/` 아래 PNG다.
- 기본 파일명은 `s<seed>_cx<cx>_cz<cz>_r<radius>.png`처럼 seed와 chunk footprint를 담는다.
- PNG의 각 pixel은 정확히 하나의 `PixelizedColumn`이다. 기본 출력 density는
  `1 world block = 1 pixel = 1 voxel column`이며, `--width`/`--height`가 다르면 renderer가 표시만
  scale하고 stage 11 column count 자체를 바꾸지 않는다. Rectangular output은 square chunk footprint를
  가로/세로로 늘이지 않고 중앙 square map viewport 안에 nearest-neighbor로 표시하며, 남는 좌우 또는
  상하 band는 neutral letterbox color로 채운다.
- color ramp는 integer `surface_y`를 기준으로 deterministic terrain height를 표시한다. ocean/lake
  standing water, river water hint, dry basin, ridge hint는 legend/metadata 또는 optional overlay/channel로
  분리해 표시할 수 있어야 한다.
- PNG metadata/stdout은 input seed, center chunk, chunk radius, chunk x/z range, world block bounds,
  column resolution, sea level, surface height min/avg/max, water column count, river-water hint count,
  dry/ridge column count, source macro-field cache key, generator version, square render viewport,
  `BoundaryCache` canonical noisy boundary overlay 여부를 기록한다.
- overlay는 chunk footprint outline, chunk grid, subtle graph Voronoi cell edge overlay, scale bar,
  small legend, topdown compass를 포함한다.
  Voronoi cell edge overlay는 raw corner-to-corner graph edge가 아니라 stage 9 `BoundaryCache`의
  canonical noisy boundary curve를 사용한다. 각 noisy polyline segment는 requested chunk/world
  footprint에 clipping한 뒤 pixelized column/border lattice로 snap하고, snapped endpoint 사이를
  4-connected orthogonal stair-step grid path로 확장한 다음 square map viewport에 투영한다. 따라서
  boundary overlay는 pixelize unit을 정확히 따라가며 diagonal stroke가 column interior를 가로지르지
  않는다. Rectangular output은 square map viewport를 중앙에 유지하고 남는 band를 neutral letterbox
  color로 채운다. 이 overlay는 height color ramp를 압도하지 않는 진단용 reference layer다.
  방향 기준은 macro field topdown과 같아서 이미지 위=N, 오른쪽=E, 아래=S, 왼쪽=W다.

### 현재 구현 상태

- 이 binary는 graph/macro/hydrology/boundary/macro-field input을 준비한 뒤 `PixelizedChunkArea`를
  렌더한다. Stage 11 column output은 `generate_pixelized_chunk_area`가 소유하고, binary의 graph access는
  metadata count와 stage 9 `BoundaryCache` canonical noisy curve overlay에 한정된다.
- 문서 계약상 `pixelize_preview`는 `heightfield_preview`보다 앞선 stage surface다. heightfield rewrite는
  이 preview가 검사한 pixelized columns를 downstream input으로 소비해야 한다.

### 검증 기준

- 같은 seed/config/chunk range는 같은 pixelized column과 PNG metadata를 만든다.
- output pixel count는 chunk footprint의 world-block column count와 일치한다.
- 모든 `surface_y`/`water_y`는 integer block height여야 한다.
- source macro masks와 `combined_macro_height`는 metadata 또는 debug dump로 추적 가능해야 한다.
- preview renderer가 `MacroFieldTile`을 직접 재샘플해 `PixelizedChunkArea`를 우회하면 회귀다.

---

## `heightfield_preview` CLI 계약

`heightfield_preview`는 stage 12 heightfield / voxel-column realization vertical slice를 chunk 생성 없이 검사하는
isometric preview binary다.

새 stage contract에서 이 preview는 stage 11 `PixelizedChunkArea` / `PixelizedColumn`을 downstream
heightfield / voxel-column cache로 변환한 뒤, column을 diagnostic box로 voxelize해서 isometric renderer에
전달한다. 실제 `ChunkData` final fill은 아니며, surface/material/vegetation stage도 아직 적용하지 않는다.
현재 구현은 compatibility vertical slice라서 `MacroFieldTile`을 직접 읽을 수 있지만, rewrite target은
`pixelize_preview`가 검사한 resolved columns를 소비하는 것이다.

### 입력

- 필수 positional 인자: `<seed> <center-x> <center-z>`
  - `center-x`, `center-z`는 기본적으로 chunk coordinate다.
  - 예전 world-block center 입력이 필요하면 `--world-center` 또는 `--world-coordinates`를 사용한다.
- 선택 인자:
  - `--width <u32>`: 기본 `1280`
  - `--height <u32>`: 기본 `720`
  - `--world-span-blocks <i32>`: 가로 footprint, 기본 `1024`
  - `--chunk-radius <i32>`: positional center chunk를 중심으로 하는 square chunk radius. 지정되면
    `--world-span-blocks` 기반 footprint 대신 `center_chunk-r .. center_chunk+r` inclusive chunk range를
    사용한다.
  - `--columns-x <u32>`: heightfield sample column 수. free-window 기본은 `1024`이고,
    `--chunk-radius` 모드 기본은 `(2r+1) * 32`다.
  - `--columns-z <u32>`: 기본은 image aspect에서 계산. 단 `--chunk-radius` 모드에서는 square
    footprint에 맞춰 기본값이 `columns-x`가 된다. 명시한 `--columns-x`/`--columns-z`는
    chunk당 기본 column 수보다 우선한다.
  - `--region-size-blocks <i32>`
  - `--site-spacing-blocks <i32>`
  - `--land-bias <f32>`
  - `--quarter-turns <u8>`: isometric camera rotation in 90 degree steps
  - `--stage heightfield`
  - `--output <path>`

### 출력

- 기본 출력은 `target/heightfield-preview/s<seed>_cx<center-x>_cz<center-z>_q<quarter>_r<radius>.png`다.
  `--output`이 명시되면 해당 경로를 그대로 사용하고 suffix를 강제로 붙이지 않는다.
- PNG에는 `new-world-preview-header` iTXt metadata chunk가 들어간다.
- metadata/stdout은 column resolution, columns-per-chunk 또는 explicit column override, sample spacing,
  block height min/avg/max, water/ocean/lake/
  river/dry/ridge column count, contour-band heightfield policy, contour minimum gap, integer height snap policy, ocean
  visible `y=0` policy, river water descent stats, meso/perlin stub 상태, isometric view/projection,
  timing을 기록한다.
- metadata/stdout은 input center, input unit, center chunk, center world block, world footprint,
  chunk x/z range, chunk radius, chunk edge blocks, macro-field tile edge blocks, column step/resolution,
  sea level과 height range를 함께 기록한다.
- overlay는 stage 이름, column resolution, surface height min/avg/max, diagnostic color key,
  primary 1024-block macro-field tile boundary key, secondary 256-block chunk-group key, chunk footprint
  outline key, scale bar, 방향 compass를 표시한다. `heightfield_preview`에서 terrain scale을
  읽는 주 grid는 `macro_field_preview`와 같은 1024-block macro tile grid다.
  Voronoi cell edge 진단 overlay는 stage 9 `BoundaryCache`의 canonical noisy boundary curve를
  cyan line으로 그린다. 이 overlay는 raw corner-to-corner graph edge나 nearest-owner raster boundary로
  대체하지 않으며, terrain을 가리지 않도록 반투명으로 legend/compass 전 단계에 렌더한다.
  boundary point는 고정된 높은 평면이 아니라 heightfield local visible top에 drape한다. water column은
  terrain bed 대신 visible water surface를 쓰고, dry terrain은 `surface_y`를 쓰며, z-fighting 방지용
  작은 lift만 더한다. metadata/stdout은
  `boundary_overlay=cyan_boundary_cache_noisy_curves_draped_visible_surface` 의미를 기록해야 한다.
  column resolution은 실제 샘플링된 column count를 뜻하며, legend에는 sample spacing과
  chunk-radius 모드의 columns-per-chunk도 함께 표시한다. per-block face outline과 정수 side-step
  line은 렌더하지 않으며, 별도 toggle 옵션도 제공하지 않는다.
- overlay와 metadata/stdout은 중앙 player diagnostic cube를 기록한다. 이 큐브는 final gameplay
  entity가 아니라 heightfield preview scale marker이며, world/block 기준 `1 x 1 x 4` block 크기,
  중앙 world position, bottom/top `y`, sampled column count를 표시해야 한다.
- legend/metadata overlay는 출력 해상도에 비례해 커져야 하며, 기본 metadata panel은 화면 높이의 약
  1/5을 차지하도록 한다. scale bar, swatch, text spacing도 같은 scale을 따라야 한다.
- 방향 compass는 `heightfield_preview`에 한해 isometric projection과 `--quarter-turns`가 적용된 뒤의
  screen-space 방향을 표시한다. 즉 N/E/S/W는 현재 quarter view에서 world cardinal 방향이 실제 화면으로
  투영된 위치에 놓인다. `macro_field_preview` 같은 topdown preview는 기존처럼 이미지 위=N,
  오른쪽=E 기준을 유지한다.

### 현재 구현 상태

- 현재 binary는 아직 compatibility path로 `MacroFieldTile`을 직접 읽는 구현일 수 있다. stage 12 rewrite
  완료 후에는 `PixelizedChunkArea`를 입력으로 받아야 하며, first chunk-aligned pixel resolve를 반복하면 안 된다.
- meso feature와 Perlin micro relief는 `0` stub이다.
- `combined_macro_height -0.5..0.0..1.0`를 `-1024..0..2048 block` signed sea-level scale로 매핑한다.
  중심 관심 구간 `-0.25..0.75`는 `-512..1536 blocks`로 읽는다. 이 scale은 macro field contour,
  heightfield resolve, downstream pixel/column preview가 공유하는 block-domain 계약이다.
- ocean/lake mask는 sea-level `y = 0` water hint가 된다. 현재 heightfield vertical slice에서는
  ocean/lake visible surface도 `y = 0`이며, bathymetry/bed depression을 preview terrain으로 렌더하지
  않는다.
- heightfield/pixelize output은 voxel-oriented preview/fill을 위해 integer block height로 snap한다.
  raw macro scalar는 diagnostic field로 보존되지만, surface/water column output은 integer `y`를 따른다.
- heightfield는 macro field contour preview와 같은 block-height scale을 사용한다. 기본 contour step은
  1 block이고 일반 land terrain의 기본 minimum gap은 0 block이다. `combined_macro_height`에서 얻은
  raw block height를 해당 contour band의 lower integer level로 quantize한다. 현재 기본값에서는 raw
  block height와 visible block height가 같은 scale을 유지한다. river corridor 기본 minimum gap도
  0 block이며, 이후 필요하면 별도 override로 다시 분리할 수 있다. smoothing/interpolation은 현재 disabled/stub이다.
  standing-water shoreline continuity clamp는 현재 제거되어 있으며 final land output은 raw block height의
  contour lower band를 보존한다. contour line segment 자체는 debug surface이며 heightfield source of truth가
  아니다.
- 일반 terrain에는 인접 column 기준 ceiling pass를 적용하지 않는다. raw/macro source가 크게 뛰면
  integer snap 뒤 visible surface도 같은 block scale로 뛰며, 그 점프는 source field 진단 대상으로 남긴다.
- ocean/lake water surface는 `y = 0`이며, standing water와 인접한 land는 heightfield post-pass에서
  grid-distance 기반 contour ceiling을 받지 않는다. launch slice에서 바다 옆 land가 즉시 높은 vertical
  cliff로 솟으면 macro/pixelize source scalar 또는 coast profile을 먼저 진단한다.
- broad river valley는 이미 `combined_macro_height`에 반영되어 있으므로 heightfield stage에서
  같은 계곡을 다시 carve하지 않는다. macro field 쪽 river guide는 selected hydrology flow/slope/bend
  context에서 broad valley와 narrow bed, bank roughness, gravel, cutbank hint를 만든다.
  heightfield는 이 중 bed-depth hint만 terrain bed/water split에 반영하고, gravel/cutbank는
  downstream surface/material diagnostic hint로 보존한다. river hint column에는 preliminary integer river
  water height를 만들고, 인접 river/standing-water surface와 한 block 이하의 step으로 천천히 내려오도록
  clamping한다.
- block color는 final material이 아니라 diagnostic terrain ramp다. water/ocean은 muted blue, low land는
  green-gray, high/ridge는 pale gray, dry basin은 muted gray/mauve 계열이다.
- player diagnostic cube는 terrain diagnostic ramp와 명확히 구분되는 형광색으로 그린다. 큐브 footprint는
  preview footprint 중앙의 world/block 기준 `1 x 1` block이고 높이는 `4` block이다. 바닥은 해당
  footprint와 가장 가까운 heightfield column/columns의 `surface_y` 최댓값에 맞춰 지형 블럭 위에
  놓이며, 지형 안에 묻히거나 공중에 떠 있으면 회귀다.
- default renderer is a CPU 2D isometric column renderer, not a tunable 3D orthographic camera. It
  uses:

```text
screen_x = (x - z) * tile_w / 2
screen_y = (x + z) * tile_h / 2 - y * vertical_px_per_block
```

  `vertical_px_per_block`은 preview 렌더링 전용 값이지만, column density 때문에 별도 세로
  normalization을 적용하지 않는다. 높이는 macro/heightfield block-domain에서 이미 산출되며,
  preview는 그 `surface_y`를 cubic block scale로 그린다. X/Z 픽셀 스케일도 별도 옵션이 아니라
  column count, footprint, image size에서 파생된다.
- The preview draws top diamonds and only visible neighbor-difference side faces without per-block
  face outlines or integer side-step lines. The visible side
  set and painter order are derived from the current `--quarter-turns` projection, not from fixed
  east/south faces. It must show top surfaces and macro relief together; a side-wall chart, a flat
  topdown plane, and quarter-specific missing back/side faces are regressions.
- The player diagnostic cube participates in the same quarter-turn painter order and visible-face
  policy as terrain columns. It should not introduce fixed east/south faces that break rotated
  previews.
- Macro-field tile boundary overlay is drawn at the generation cache tile scale, currently 1024
  blocks, and is the primary readable grid so the scale matches `macro_field_preview`. The
  heightfield preview draws only the chunk-aligned preview footprint outline instead of every
  32-block internal chunk line, because a dense minor grid can read as noisy surface detail. A
  secondary major chunk-group grid is drawn every 256 blocks, equal to 8 chunks, but it must not
  visually dominate the 1024-block macro tile grid. These lines are diagnostic overlays, not terrain
  features.
- `--chunk-radius r` is a square chunk-coordinate footprint, not separate x/z radii. It includes the
  center chunk and covers `2r+1` chunks on each horizontal axis. A radius of `0` previews one chunk.

### 검증 기준

- 같은 seed/config는 같은 column과 metadata를 만든다.
- output image는 blank가 아니어야 한다.
- water mask가 있는 column은 water level hint를 가져야 한다.
- meso/perlin stub 값은 0이어야 한다.
- surface/water height output은 integer block height여야 한다.
- coast-adjacent land는 heightfield shoreline continuity clamp 없이 macro/pixelize source height를 보존해야 한다.

---

## `generation_preview_suite` CLI 계약

`generation_preview_suite`는 generation stage를 직접 렌더하지 않는 취합 binary다. 기존 preview
binary들을 순서대로 실행하고, 결과 PNG를 하나의 output directory에 짧은 ordered filename으로 저장한다.

### 입력

- 필수 positional 인자: `<seed>`
- 선택 인자:
  - `--center-chunk-x <i32>` / `--cx <i32>`: 기본 `0`
  - `--center-chunk-z <i32>` / `--cz <i32>`: 기본 `0`
  - `--radius <i32>` / `--r <i32>`: positive chunk-radius, 기본 `8`.
  - `--output <path>`: 기본 `target/generation-preview-suite/s<seed>_cx<cx>_cz<cz>_r<r>`
  - `--overview-width <u32>`, `--overview-height <u32>`
  - `--zoom-width <u32>`, `--zoom-height <u32>`
  - `--heightfield-width <u32>`, `--heightfield-height <u32>`
  - `--contour-step <i32>`: 기본 `8`

### 좌표 계약

- suite의 center는 chunk coordinate다.
- graph/macro/biome overview child는 world-block center를 받으므로 suite가
  `center_chunk * CHUNK_EDGE + CHUNK_EDGE / 2`로 변환한다.
- `pixelize_preview`와 `heightfield_preview`는 chunk center/radius를 그대로 받는다.
- zoomed `macro_field_preview`는 변환된 world-block center와 `--chunk-radius <r>`를 함께 받는다.

### 출력 순서

1. `01_graph_cont.png`: `graph_voronoi_preview --mode continentality`
2. `02_graph_elev.png`: `graph_voronoi_preview --mode elevation`
3. `03_macro_map.png`: `macro_map_preview`
4. `04_biome_map.png`: `biome_map_preview`
5. `05_macro_combined.png`: `macro_field_preview --channel combined --contours --contour-step 8`
6. `06_macro_zoom.png`: 같은 macro field combined/contour view를 chunk radius footprint로 zoom
7. `07_pixelize.png`: 같은 chunk center/radius의 `pixelize_preview`
8. `08_heightfield.png`: 같은 chunk center/radius의 `heightfield_preview`

### 불변식

1. suite는 terrain policy나 preview renderer를 복제하지 않는다.
2. child preview 중 하나가 실패하면 suite도 실패해야 한다.
3. ordered filename은 비교와 보고를 위해 안정적이어야 한다.
4. child binary가 sibling executable로 존재하면 그것을 우선 실행하고, 없으면 `cargo run --bin`으로
   fallback할 수 있다.

---

## Determinism

- 같은 seed, generator version, area, stage input은 같은 preview를 만든다.
- preview는 chunk 생성 없이도 stage 결과를 검사할 수 있어야 한다.
- 이미지 출력뿐 아니라 stage별 raw dump가 필요하면 같은 deterministic 입력 계약을 사용한다.
- stage artifact를 발견하면 문서, 테스트, 구현 중 어느 층의 가정이 깨졌는지 추적한다.

---

## Artifact 기준

- graph region 사각 경계가 보이면 회귀다.
- raw Voronoi edge가 그대로 river/coast/biome boundary로 보이면 회귀다.
- noisy boundary가 guard 영역 밖으로 나가면 회귀다.
- hydrology flow가 outlet 없이 끊기면 회귀다.
- biome/material transition이 hard owner 선을 그대로 따라가면 회귀다.
- macro field channel이 graph-derived source와 무관한 새 noise처럼 보이면 회귀다.
- lit heightfield preview가 단색 평면이거나 lighting 방향을 읽을 수 없으면 회귀다.

---

## 불변식

1. preview binary는 stage 결과를 chunk 생성 없이 검사할 수 있어야 한다.
2. preview output은 deterministic이어야 한다.
3. preview는 문서와 테스트의 보조물이 아니라 generation artifact를 발견하는 1차 검증 표면이다.
4. 이상한 작은 흔적이 보이면 무시하지 않고 source-of-truth 문서와 테스트로 환류한다.
5. 매 generation stage는 전용 preview binary 또는 기존 binary의 명시적 stage/mode로 검사 가능해야 한다.
6. macro field/pixelize 이후 final heightfield 검증은 흰색 texture와 단순 lighting이 있는 top-down rendering을
   포함해야 한다.
