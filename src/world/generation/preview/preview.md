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
5. mountain/ridge/fault edge and coast edge preview
6. dominant site map
7. blended influence map
8. elevation/corner downhill arrow map
9. watershed map
10. river flow accumulation map
11. noisy edge preview
12. Voronoi-derived macro noise/gradient map preview
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
  owner region count, mode/map name을 함께 기록한다.
- 각 PNG는 작은 legend overlay를 가진다. field map은 gradient color bar와 양끝 의미 label을 표시하고,
  identity map은 간단한 header만 표시한다.
- 픽셀 생성은 Rayon 병렬 chunk 처리로 수행한다.

### 현재 구현 상태

- `graph_voronoi_preview`는 `world::generation::graph::generate_voronoi_graph_patch(...)`를 호출해
  같은 graph contract를 시각화한다.
- binary는 graph 의미를 새로 만들지 않고, preview window에서 필요한 padding을 계산한 뒤
  `VoronoiGraphPatchRequest`를 구성한다.
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
- ridge candidate edge는 흰색, fault candidate edge는 붉은색, mountain candidate edge는 ochre,
  coast candidate edge는 sandy color overlay로 표시한다.
- stage 3 macro_map preview는 selected river나 pre-hydrology river corridor를 표시하지 않는다.
  selected river overlay는 hydrology preview가 확정한다.
- 작은 legend overlay는 elevation gradient와 ridge/fault/mountain/coast edge key를 포함한다.
- width, height, generator version, stage, world span, site spacing은 파일명에 넣지 않고 PNG
  metadata에만 기록한다.
- PNG에는 `new-world-preview-header` iTXt metadata chunk가 들어가며 graph area, site count,
  candidate edge count, coast/mountain/ridge/fault edge count, sea level, stage 4 guide input note,
  source note를 함께 기록한다.
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
- coast/ridge/fault/mountain candidate overlay는 nearest-site fill 근사 경계에 맞추지 않고,
  `MacroEdge.corners`가 참조하는 graph patch의 실제 `VoronoiCorner.position` 두 점을 world-space에서
  clipping한 뒤 픽셀 중심 좌표계로 투영한 선분으로 그린다.
- 픽셀 sampling과 PNG encoding은 preview binary 책임이다.

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

---

## 불변식

1. preview binary는 stage 결과를 chunk 생성 없이 검사할 수 있어야 한다.
2. preview output은 deterministic이어야 한다.
3. preview는 문서와 테스트의 보조물이 아니라 generation artifact를 발견하는 1차 검증 표면이다.
4. 이상한 작은 흔적이 보이면 무시하지 않고 source-of-truth 문서와 테스트로 환류한다.
