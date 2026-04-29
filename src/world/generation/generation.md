# generation

## 역할

`world::generation`은 새 graph-first 월드 생성 파이프라인을 소유한다.

이 모듈은 기존 `atlas cell` / `chunk` 중심 생성기를 바로 지우지 않고, 새 구조를 단계별로
대체하기 위한 경계다. 런타임 호환이 필요한 동안에는 `src/world/legacy/generation`의 공개 API를
`world::generation::*`로 다시 내보낸다. 새 코드는 가능한 한 이 모듈 아래의 graph-first leaf
모듈에 추가한다.

---

## 책임

- Voronoi graph 기반 macro terrain 생성 단계 소유
- graph region, site, corner, edge 기반의 deterministic 생성 계약
- 대륙/바다 ownership, macro elevation, 산맥/능선/단층/해안 guide의 생성 순서 정의
- edge 기반 hydrology, watershed, river 후보망의 생성 순서 정의
- noisy boundary, continuous field, heightfield synthesis, voxel fill로 이어지는 단계 경계 정의
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

## 하위 모듈

현재 구현된 scaffold:

- `graph/graph.md`: graph region, Voronoi site/corner/edge id와 patch 계약
- `field/field.md`: hard owner가 아닌 continuous blended field sampling 계약
- `hydrology/hydrology.md`: watershed, drainage node, selected river edge 계약
- `pipeline/pipeline.md`: graph-first stage order와 column synthesis scaffold

계획된 leaf module:

- `macro_map`: continent/ocean ownership, Voronoi macro elevation, mountain/ridge/fault/coast guide
- `boundary`: noisy coast/river/biome/fault boundary realization
- `heightfield`: Voronoi-derived macro map과 Perlin micro relief 합성
- `surface_plan`: biome, material, water/coast/wetland policy resolve
- `voxel`: column plan을 `ChunkData`로 채우는 graph-first voxel fill
- `preview`: stage별 topdown preview binary 입력/출력 계약

빈 leaf 구현 파일은 만들지 않는다. 새 leaf를 실제로 구현할 때 같은 폴더에 대응 문서를 먼저 두고,
그 문서가 소유권과 불변식을 정의해야 한다.

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

## Preview 계약

각 generation stage는 독립적으로 실행 가능한 topdown preview binary에서 검사 가능해야 한다.

- 같은 seed, generator version, area, stage input은 같은 preview를 만든다.
- preview는 chunk 생성 없이도 stage 결과를 볼 수 있어야 한다.
- 이미지 출력뿐 아니라 stage별 raw dump가 필요하면 같은 deterministic 입력 계약을 사용한다.
- stage artifact를 발견하면 문서, 테스트, 구현 중 어느 층의 가정이 깨졌는지 추적한다.

초기 preview 우선순위:

1. graph region/site/corner/edge map
2. continent/ocean ownership
3. Voronoi macro elevation
4. mountain/ridge/fault/coast edge guide
5. hydrology downhill/watershed/flow accumulation
6. noisy edge realization
7. blended field map
8. Voronoi-derived macro noise/gradient map
9. Perlin micro relief
10. final heightfield
11. biome/material map

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
