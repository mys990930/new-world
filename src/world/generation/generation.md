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
- polygon 경계는 후보선이자 소유권 경계일 수 있지만, 그대로 보이는 선이어서는 안 된다.
- 이 프로젝트는 작은 island가 아니라 큰 대륙과 바다, 대륙 내부 산맥과 강을 목표로 한다.

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
- 대륙/바다 ownership, macro elevation, 산맥/능선/단층/해안 guide의 생성 순서 정의
- edge 기반 hydrology, watershed, river 후보망의 생성 순서 정의
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
있어야 한다.

1. seed 기반 padded Voronoi/Delaunay dual graph를 생성한다.
2. site/corner에 base temperature, humidity, continentality, elevation seed를 부여하고 이웃 graph를 참고해 smoothing한다.
3. continent/ocean basin ownership과 signed macro elevation을 만든다.
4. macro elevation, gradient, continent/coast context를 읽어 ridge/fault/mountain/coast edge guide를 선정한다.
5. macro elevation과 edge guide를 읽어 hydrology를 푼다: downhill, graph-stage local minima, lake/sink/outlet carve, watershed, flow accumulation, selected river chain.
6. visible feature edge만 noisy boundary로 현실화한다. raw graph topology는 그대로 보존한다.
7. graph guide, hydrology, noisy boundary를 합쳐 Voronoi-derived macro field/noise map을 만든다.
8. meso feature plan을 만든다. 이 단계는 crater, ravine, dune field, hill cluster, terrace 같은 국소 지형 객체를 feature id와 world-space anchor로 배치한다.
9. seed 기반 Perlin micro relief를 만들고 hydrology/coast/lake/ridge/meso mask로 amplitude를 제한한다.
10. macro map, meso feature deformation, hydrology valley/lake/coast constraint, noisy boundary, Perlin micro relief를 합성해 heightfield와 water surface 후보를 만든다.
11. elevation, water proximity, rain shadow, hydrology role을 반영해 final temperature/hydration/biome influence를 resolve한다.
12. biome/material/water/coast surface plan을 만든다.
13. vegetation/feature placement plan을 만든다.
14. heightfield, water, surface, vegetation plan을 한 번에 `ChunkData`로 voxel fill한다.

---

## 하위 모듈

현재 구현된 scaffold:

- `graph/graph.md`: graph region, Voronoi site/corner/edge id와 patch 계약
- `field/field.md`: hard owner가 아닌 continuous blended field sampling 계약
- `hydrology/hydrology.md`: watershed, drainage node, selected river edge 계약
- `pipeline/pipeline.md`: graph-first stage order와 column synthesis scaffold

현재 구현된 graph-first processing:

- stage 1 padded Voronoi-style graph patch 생성: deterministic jittered grid site, barycentric corner,
  site/corner-linked edge topology를 rayon 병렬 생성 뒤 id 정렬/dedup한다.
- stage 2 base graph field: site raw seed field와 smoothed field를 생성하고, corner field/elevation
  seed를 주변 site 기반으로 안정적으로 계산한다.
- stage 3/4 macro map: `generate_macro_map`이 graph patch를 입력으로 받아 continent/ocean basin
  ownership, signed macro elevation, coastness, mountainness/ridgeness, basinness, coast/ridge/fault/river-candidate
  edge guide를 별도 annotation layer로 생성한다. river guide는 routing 확정이 아니라 hydrology 전
  후보 surface다.

문서화된 다음 leaf:

- `boundary/boundary.md`: noisy coast/river/biome/fault boundary realization
- `meso_feature/meso_feature.md`: 국소 지형 feature planning과 heightfield deformation 계약
- `heightfield/heightfield.md`: Voronoi-derived macro map과 Perlin micro relief 합성
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
- selected river는 lake/sink/outlet 처리 없이 끊기지 않는다.
- local minima는 lake, sink, outlet carve 중 하나로 명시된다.
- river width는 flow와 안정적으로 연결된다.

### Field Continuity

- 인접 site의 temperature/hydration/elevation bias는 비현실적으로 튀지 않는다.
- base graph field smoothing은 raw seed 대비 인접 site 차이를 줄여야 한다.
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
2. macro elevation은 Voronoi graph 기반으로 먼저 생성되고, Perlin은 마지막 micro relief로만 합성된다.
3. 산맥/능선/단층/해안 edge guide는 hydrology보다 먼저 정해진다.
4. hydrology는 최종 heightfield와 voxel fill 전에 valley/lake/coast 제약을 제공한다.
5. noisy boundary는 visible feature edge의 realization layer이며 raw graph topology를 대체하지 않는다.
6. meso feature는 macro ownership을 뒤집지 않고 heightfield가 읽을 deterministic deformation plan을 제공한다.
7. polygon owner와 visible material/biome boundary는 분리될 수 있어야 한다.
8. material, water, vegetation은 직접 `ChunkData`를 수정하지 않고 plan으로 합쳐진 뒤 voxel fill에서 반영된다.
9. 각 stage는 topdown preview binary로 진단 가능해야 한다.
10. legacy generation re-export는 migration bridge이며, 새 graph-first 책임을 legacy 쪽으로 늘리지 않는다.
