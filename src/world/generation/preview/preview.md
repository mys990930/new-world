# preview

## 역할

`preview`는 generation stage별 topdown preview binary의 입력/출력 계약을 소유한다.

초기 generator에서는 기능 완성도만큼 진단 가능성이 중요하다. 각 stage는 chunk 생성 없이도 같은
입력으로 같은 preview를 만들 수 있어야 한다.

---

## 책임

- stage별 preview input area, seed, generator version, config 계약
- deterministic PNG 또는 raw dump 출력 계약
- graph, macro map, hydrology, boundary, field, heightfield, surface 결과를 독립적으로 검사할 수 있는 surface 정의
- artifact regression을 테스트와 연결할 수 있는 기준 제공

---

## 비책임

- GPU rendering
- interactive editor UI
- runtime chunk storage mutation
- generation stage 의미 소유

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
11. noisy edge preview
12. macro field rasterization preview
13. meso feature plan preview
14. Perlin micro relief preview
15. heightfield and water surface preview
16. final temperature/hydration/biome influence preview
17. biome/material/surface plan preview
18. vegetation placement preview
19. voxel fill preview

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
- stage 7 boundary overlay는 모든 graph Voronoi edge의 canonical noisy curve를 그린다. 이 overlay는
  straight edge를 재샘플링한 선이 아니라 endpoint는 유지하고 중간 point를 edge normal 방향으로
  흔든 울퉁불퉁한 polyline이어야 한다. boundary curve는 독립 jitter가 아니라 correlated wave/value
  noise와 smoothing을 거친 자연스러운 곡선이어야 하며, 기본 4K preview와 640 smoke preview에서도
  식별 가능한 amplitude를 가져야 한다.
- ridge candidate edge는 흰색, fault candidate edge는 붉은색, coast candidate edge는 sandy color overlay로 표시한다.
- macro_map preview는 macro_map fill과 stage 4/5 guide를 기본 layer로 표시하고, stage 6 hydrology가
  구현된 뒤에는 같은 composite 위에 selected river result를 추가 overlay한다.
- selected river는 hydrology가 확정한 `GraphRiverSegment`만 표시한다. pre-hydrology river candidate나
  macro_map river potential은 표시하지 않는다.
- selected river segment는 stage 7 `BoundaryCache`가 제공하는 canonical noisy edge geometry를 따라 cyan/blue line으로 표시하고,
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
- hydrology overlay는 stage 7 이후 같은 edge id의 canonical noisy curve를 사용한다. 따라서 selected river는 raw
  nearest-site raster boundary가 아니라 hydrology가 선택한 Voronoi edge chain의 noisy geometry 위에 놓인다.
- hydrology overlay는 macro_map의 명시적 lake edge class를 통과한 selected segment만 그린다. 따라서
  nearest-site lake fill 경계와 corner surface 기준이 어긋나도 selected river가 lake boundary/internal/
  adjacent edge 위에 그려지면 회귀다.
- 픽셀 sampling과 PNG encoding은 preview binary 책임이다.

---

## `macro_field_preview` CLI 계약

`macro_field_preview`는 stage 8 macro field rasterization을 chunk 생성 없이 검사하는 topdown preview
binary다.

macro field는 noise source가 아니다. 이 preview는 graph/macro/hydrology/boundary cache를
world-space sample grid로 굽는 과정을 검사한다. source of truth는 graph topology, macro annotation,
selected hydrology result, canonical noisy boundary에 남고, `MacroFieldTile`은 heightfield와 chunk fill이
빠르게 읽기 위한 graph-derived signed distance / influence field cache다.

preview와 runtime cache miss는 ridge/coast/river curve distance를 sample마다 반복 계산하지 않아야
한다. stage 8 preview는 먼저 selected river, ridge, coast의 canonical noisy curve를 tile source
pixel로 rasterize하고, distance propagation으로 influence field를 만든 뒤 그 결과를 렌더한다.
ownership/mask의 noisy-boundary side 판정은 정확도 유지를 위해 launch 단계에서 per-sample query가
남을 수 있지만, 이 비용은 chunk fill hot path가 아니라 macro field tile cache miss에 한정된다.

### 입력

- 필수 positional 인자: `<seed> <center-x> <center-z>`
  - `center-x`, `center-z`는 world-block 좌표다.
- 선택 인자:
  - `--width <u32>`: 기본 `3840`
  - `--height <u32>`: 기본 `2160`
  - `--world-span-blocks <i32>`: 이미지 가로가 덮는 world-block 폭, 기본 `32768`
  - `--stage macro_field`
  - `--channel <all|macro|mask|ridge|river|combined|lit|contour>`: 기본 `lit`
  - `--contour-step <blocks>`: contour channel과 overlay가 사용할 block-height 간격, 기본 `8`
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
- `river`: selected hydrology segment가 참조하는 canonical noisy curve 주변 distance, flow,
  carve strength. 이 channel은 selected curve를 source pixel로 rasterize한 tile influence pass를
  사용해야 하며, raw polyline distance를 preview pixel마다 반복 계산하면 안 된다.
- `combined`: Perlin 합성 전 macro elevation + ridge raise - river carve - coast/lake flatten 결과.
  이 단계의 river carve는 최종 water/voxel carve가 아니라 heightfield가 읽을 2D valley guide이며,
  combined/lit preview에서 보여야 한다. `combined`는 진단용 heat map이 아니라 macro base 위에 ridge와
  river carve가 얹힌 pre-Perlin terrain surface로 읽히도록 subtle terrain ramp를 사용한다.
- `lit`: combined macro height 또는 heightfield stage output을 흰색 texture와 단순 lighting으로
  보여주는 top-down rendering
- `contour`: heightfield 직전 `combined_macro_height`를 block-height scale으로 변환한 뒤 Marching
  Squares로 추출한 contour line preview. 기본 level step은 8 blocks이며, 5 level마다 major contour를
  그린다. sea level `y=0` contour는 별도 blue 계열로 표시한다.

중간 단계는 2D gradient/mask preview여야 한다. 최종 산출물은 색상 지형도가 아니라 흰색 texture에
간단한 normal/light shading을 입힌 top-down heightfield rendering이어야 한다. lighting은 진단용이며
renderer/GPU 계약을 만들지 않는다.

### 출력

- 기본 출력은 `target/macro-field-preview/` 아래 PNG다.
- 기본 파일명은 `s<seed>_x<center-x>_z<center-z>_<channel>.png`처럼 짧게 유지한다.
- width, height, generator version, stage, world span, tile resolution은 파일명에 넣지 않고 PNG
  metadata에만 기록한다.
- PNG에는 `new-world-preview-header` iTXt metadata chunk가 들어간다.
- metadata/stdout은 tile bounds, sample resolution, source graph/macro/hydrology/boundary version,
  channel name, min/max/avg, absolute preview scale, robust percentile diagnostic range, noisy
  boundary displacement stats, finite/NaN count, overlap guard width, source cache key, legend
  labels, influence source curve/pixel count, tile generation timing을 기록한다.
- 각 PNG는 작은 legend overlay를 가진다. gradient channel은 color bar와 low/high 의미를 표시하고,
  mask channel은 ocean/lake/dry/coast/land key를 서로 구분되는 색으로 표시한다. lit heightfield는 height range와 light
  direction만 표시한다. contour channel은 minor/major/sea-level key와 contour step/major spacing을 표시한다.
- 각 PNG는 별도 방향 compass overlay를 포함한다. 방향 기준은 모든 topdown macro field preview와
  같아서 위=N, 오른쪽=E, 아래=S, 왼쪽=W다.
- 모든 `macro_field_preview` channel은 stage 7 `BoundaryCache`의 canonical noisy Voronoi graph edge
  overlay를 표시한다. 이 overlay가 사용자가 요청한 terrain tile/boundary 확인의 기본 표면이지만,
  field 값을 압도하면 안 된다. 기본 스타일은 위치 참고용 faint overlay이며, 색과 opacity는
  macro/combined/lit 값을 먼저 읽을 수 있을 정도로 약해야 한다.
- `lit` channel은 broad hillshade가 우선 읽혀야 하므로 다른 channel보다 더 희미한 Voronoi edge
  overlay를 사용한다. lit에서 edge가 조명/고저차보다 먼저 보이면 회귀다.
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

### 검증 기준

- determinism: 같은 seed/config/tile/channel은 같은 PNG와 metadata를 만든다.
- adjacent tile overlap stability: 인접 tile overlap의 같은 world-space sample은 같은 값을 가진다.
- preview scale continuity: `macro`, `combined`, `lit` channel은 preview image나 tile마다 local
  min/max를 다시 잡지 않고 문서화된 absolute normalized scale로 렌더해야 한다.
- finite/range sanity: 모든 channel은 finite 값이며 문서화된 range를 벗어나지 않는다.
- hydrology endpoint attachment: river valley field는 selected segment의 canonical noisy curve와
  lake inlet/outlet endpoint를 따라가야 한다.
- noisy boundary ownership: macro elevation과 mask 경계는 nearest-site straight boundary가 아니라
  stage 7 noisy boundary curve를 따라야 한다.
- no lake-edge river invariant: river valley field는 `MacroLakeEdgeClass::NonLake` selected segment만
  rasterize해야 한다.
- preview nonblank: 각 channel은 blank 단색 이미지가 아니어야 하며 legend와 metadata를 포함해야 한다.
- contour sanity: contour segment는 finite world-space endpoint를 가져야 하며, flat field는 contour를
  만들지 않고 simple ramp field는 crossing contour를 만들어야 한다.

---

## `heightfield_preview` CLI 계약

`heightfield_preview`는 stage 11 heightfield / water surface vertical slice를 chunk 생성 없이 검사하는
isometric preview binary다.

이 preview는 `MacroFieldTile`을 `HeightfieldTile` column cache로 변환한 뒤, column을 diagnostic box로
voxelize해서 isometric camera를 가진 offscreen renderer에 전달한다. 실제 `ChunkData` final fill은 아니며,
surface/material/vegetation stage도 아직 적용하지 않는다.

### 입력

- 필수 positional 인자: `<seed> <center-x> <center-z>`
  - `center-x`, `center-z`는 world-block 좌표다.
- 선택 인자:
  - `--width <u32>`: 기본 `1280`
  - `--height <u32>`: 기본 `720`
  - `--world-span-blocks <i32>`: 가로 footprint, 기본 `8192`
  - `--chunk-radius <i32>`: `center-x/center-z` world block이 속한 chunk를 중심으로 하는 square
    chunk radius. 지정되면 `--world-span-blocks` 기반 footprint 대신
    `center_chunk-r .. center_chunk+r` inclusive chunk range를 사용한다.
  - `--columns-x <u32>`: heightfield sample column 수, 기본 `192`
  - `--columns-z <u32>`: 기본은 image aspect에서 계산. 단 `--chunk-radius` 모드에서는 square
    footprint에 맞춰 기본값이 `columns-x`가 된다.
  - `--xz-scale <u32>`: 같은 world footprint에서 X/Z column density만 곱하는 multiplier, 기본 `2`.
    `--horizontal-subdivisions` alias도 허용한다. `2`이면 effective columns가 각 축 두 배가 되고
    effective sample spacing은 절반이 되지만, Y height block 값은 바뀌지 않는다.
  - `--region-size-blocks <i32>`
  - `--site-spacing-blocks <i32>`
  - `--land-bias <f32>`
  - `--quarter-turns <u8>`: isometric camera rotation in 90 degree steps
  - `--vertical-scale <f32>`: automatic vertical relief fit multiplier, 기본 `1.0`
  - `--stage heightfield`
  - `--output <path>`

### 출력

- 기본 출력은 `target/heightfield-preview/s<seed>_x<center-x>_z<center-z>.png`다.
- PNG에는 `new-world-preview-header` iTXt metadata chunk가 들어간다.
- metadata/stdout은 base/effective column resolution, XZ scale, base/effective sample spacing,
  block height min/avg/max, water/ocean/lake/
  river/dry/ridge column count, contour-band heightfield policy, integer height snap policy, ocean
  visible `y=0` policy, river water descent stats, meso/perlin stub 상태, isometric view/projection,
  timing을 기록한다.
- metadata/stdout은 `center-x/center-z`, world footprint, chunk x/z range, chunk radius,
  chunk edge blocks, macro-field tile edge blocks, column step/resolution, sea level과 height range를
  함께 기록한다.
- overlay는 stage 이름, column resolution, surface height min/avg/max, diagnostic color key,
  primary 1024-block macro-field tile boundary key, secondary 256-block chunk-group key, very faint
  32-block chunk boundary key, scale bar, 방향 compass를 표시한다. `heightfield_preview`에서 terrain scale을
  읽는 주 grid는 `macro_field_preview`와 같은 1024-block macro tile grid다.
  column resolution은 effective column count를 뜻하며, legend에는 `XZ<n>`과 base/effective spacing도
  함께 표시한다.
- legend/metadata overlay는 출력 해상도에 비례해 커져야 하며, 기본 metadata panel은 화면 높이의 약
  1/5을 차지하도록 한다. scale bar, swatch, text spacing도 같은 scale을 따라야 한다.
- 방향 compass는 이미지 위=N(`world -Z`), 오른쪽=E(`world +X`), 아래=S, 왼쪽=W라는 macro field
  기준을 유지한다. isometric preview에서도 이 표기는 화면/월드 topdown 기준의 등록 보조 overlay이며,
  height 값을 바꾸지 않는다.

### 현재 구현 상태

- meso feature와 Perlin micro relief는 `0` stub이다.
- `combined_macro_height -0.75..0.0..1.25`를 `-48..0..160 block` signed sea-level scale로 매핑한다.
- ocean/lake mask는 sea-level `y = 0` water hint가 된다. 현재 heightfield vertical slice에서는
  ocean/lake visible surface도 `y = 0`이며, bathymetry/bed depression을 preview terrain으로 렌더하지
  않는다.
- heightfield output은 voxel-oriented preview/fill을 위해 integer block height로 snap한다. raw
  macro scalar는 diagnostic field로 보존되지만, surface/water column output은 integer `y`를 따른다.
- heightfield는 macro field contour preview와 같은 block-height scale을 사용한다. 기본 contour step은
  1 block이며, `combined_macro_height`에서 얻은 raw block height를 직접 final surface로 쓰지 않고
  해당 contour step의 lower band로 quantize한다. smoothing/interpolation은 현재 disabled/stub이다.
  water/shoreline constraint는 sea-level safety pass로 유지하되 final land output은 constraint 뒤에도
  contour step에 snap된다. contour line segment 자체는 debug surface이며 heightfield source of truth가
  아니다.
- ocean/lake water surface는 `y = 0`이며, standing water와 인접한 land는 grid-distance 기반
  contour ceiling으로 `0, 1, 2, ...` 계단을 따라 올라가야 한다. explicit cliff/meso feature가 없는
  launch slice에서 바다 옆 land가 즉시 높은 vertical cliff로 솟으면 회귀다.
- river valley는 이미 `combined_macro_height`에 carve guide로 반영되어 있으므로 heightfield stage에서
  중복 carve하지 않는다. 다만 river hint column에는 preliminary integer river water height를 만들고,
  인접 river/standing-water surface와 한 block 이하의 step으로 천천히 내려오도록 clamping한다.
- block color는 final material이 아니라 diagnostic terrain ramp다. water/ocean은 muted blue, low land는
  green-gray, high/ridge는 pale gray, dry basin은 muted gray/mauve 계열이다.
- default renderer is a CPU 2D isometric column renderer, not a tunable 3D orthographic camera. It
  uses:

```text
screen_x = (x - z) * tile_w / 2
screen_y = (x + z) * tile_h / 2 - y * vertical_px_per_block
```

  `vertical_px_per_block` is fitted so the visible height range occupies about 20-35% of the output
  height. `--vertical-scale` multiplies that automatic fit, and `--quarter-turns` rotates the
  horizontal grid without changing height data.
- The preview draws top diamonds and only visible neighbor-difference side faces. It must show top
  surfaces and macro relief together; a side-wall chart and a flat topdown plane are both regressions.
- Macro-field tile boundary overlay is drawn at the generation cache tile scale, currently 1024
  blocks, and is the primary readable grid so the scale matches `macro_field_preview`. Chunk
  boundary overlay is still drawn at the runtime chunk size (`CHUNK_EDGE`, currently 32 blocks) as a
  very faint minor grid. A secondary major chunk-group grid is drawn every 256 blocks, equal to
  8 chunks, but it must not visually dominate the 1024-block macro tile grid. These lines are
  diagnostic overlays, not terrain features.
- `--chunk-radius r` is a square chunk-coordinate footprint, not separate x/z radii. It includes the
  center chunk and covers `2r+1` chunks on each horizontal axis. A radius of `0` previews one chunk.

### 검증 기준

- 같은 seed/config는 같은 column과 metadata를 만든다.
- output image는 blank가 아니어야 한다.
- water mask가 있는 column은 water level hint를 가져야 한다.
- meso/perlin stub 값은 0이어야 한다.
- surface/water height output은 integer block height여야 한다.
- coast-adjacent land는 sea level에서 완만히 올라가는 ramp를 가져야 한다.

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
6. macro field 이후 final heightfield 검증은 흰색 texture와 단순 lighting이 있는 top-down rendering을
   포함해야 한다.
