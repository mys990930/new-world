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
- noisy boundary, continuous field, heightfield synthesis, surface plan, voxel fill 단계 경계 정의
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

## 처리 순서

generation pipeline은 chunk를 지형 정체성의 기준으로 쓰지 않는다. chunk는 출력 window이며,
macro terrain identity는 graph와 field가 소유한다.

1. 요청된 world-space x/z 범위를 덮는 padded graph region area를 계산한다.
2. deterministic site 후보를 만들고 안정화한다.
3. Voronoi/Delaunay dual graph patch를 만든다.
4. 대륙/바다 ownership과 Voronoi 기반 macro elevation을 만든다.
5. edge 기반 mountain/ridge/fault/coast guide를 먼저 정한다.
6. macro elevation과 edge guide를 읽어 hydrology 후보망을 설정한다.
7. downhill routing, local minima, lake/sink/outlet carve를 처리한다.
8. watershed, flow accumulation, selected river edge chain을 만든다.
9. river, coast, biome transition, cliff/fault boundary를 noisy boundary로 현실화한다.
10. graph-derived guide를 합쳐 Voronoi macro noise/gradient map을 만든다.
11. column별 blended climate/hydration/biome influence field를 만든다.
12. Perlin micro relief를 마지막에 합성해 final heightfield를 만든다.
13. river valley, lake flattening, wetland, coast terrace를 column plan에 반영한다.
14. biome/material/surface policy를 resolve한다.
15. `ChunkData`로 voxel fill한다.

---

## 하위 모듈

현재 구현된 scaffold:

- `graph/graph.md`: graph region, Voronoi site/corner/edge id와 patch 계약
- `field/field.md`: hard owner가 아닌 continuous blended field sampling 계약
- `hydrology/hydrology.md`: watershed, drainage node, selected river edge 계약
- `pipeline/pipeline.md`: graph-first stage order와 column synthesis scaffold

문서화된 다음 leaf:

- `macro_map/macro_map.md`: continent/ocean ownership, Voronoi macro elevation, mountain/ridge/fault/coast guide
- `boundary/boundary.md`: noisy coast/river/biome/fault boundary realization
- `heightfield/heightfield.md`: Voronoi-derived macro map과 Perlin micro relief 합성
- `surface_plan/surface_plan.md`: biome, material, water/coast/wetland policy resolve
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
5. polygon owner와 visible material/biome boundary는 분리될 수 있어야 한다.
6. 각 stage는 topdown preview binary로 진단 가능해야 한다.
7. legacy generation re-export는 migration bridge이며, 새 graph-first 책임을 legacy 쪽으로 늘리지 않는다.
