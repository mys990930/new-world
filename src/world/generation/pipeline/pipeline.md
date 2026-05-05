# pipeline

## 역할

`pipeline`은 graph-first world generation의 stage order와 column synthesis scaffold를 정의한다.

이 모듈은 새 파이프라인이 legacy generator를 대체하기 전까지 compile-time stage contract를
제공한다. 실제 stage 구현은 `graph`, `macro_map`, `hydrology`, `boundary`, `field`,
`meso_feature`, `heightfield`, `surface_plan`, `voxel`, `preview` 문서와 구현으로 분산된다.

---

## 책임

- graph-first generation stage 이름 정의
- initial generation config surface 정의
- chunk voxelization이 나중에 소비할 column synthesis request/result shape 정의
- stage 순서가 macro guide, hydrology, heightfield, voxel fill 순서를 어기지 않도록 고정

---

## 비책임

- Voronoi site 실제 생성
- noise function 실행
- hydrology solving
- surface material 선택
- `ChunkData`에 block 배치

---

## Stage Order

현재 scaffold stage는 목표 pipeline 순서를 그대로 드러낸다.

1. padded Voronoi graph
2. macro-friendly base graph fields
3. graph-field-based macro ownership/elevation resolve
4. ridge/fault guide selection
5. coast edge guide selection
6. hydrology solve
7. noisy boundary realization
8. graph-derived macro map
9. meso feature planning
10. Perlin micro relief
11. heightfield and water surface
12. climate/hydration/biome resolve
13. surface plan
14. vegetation plan
15. voxel fill

pipeline은 더 세분화될 수 있지만, 반드시 아래 대원칙을 지켜야 한다.

- graph construction이 먼저다.
- base climate와 elevation seed는 graph 단계에서 시작하지만, final temperature/hydration은 heightfield와 hydrology 이후에 다시 resolve한다.
- base `continentality`와 `elevation_seed`는 macro_map ownership/elevation resolve의 source of truth다.
- macro_map은 독자적인 continent/island noise source를 만들지 않고 graph base field를 해석한다.
- continent/ocean/island ownership과 macro elevation은 Perlin보다 먼저다.
- ridge/fault edge guide와 coast edge guide는 hydrology보다 먼저다. mountainness/rugged context는 이 guide를 고르는 입력이다.
- hydrology는 final heightfield와 voxel fill보다 먼저다.
- hydrology는 potential river guide가 아니라 selected river chain, flow accumulation, lake/sink/outlet resolution을 만든다.
- noisy boundary는 visible feature edge의 realization layer이며 raw graph topology를 대체하지 않는다.
- Voronoi-derived macro map은 graph guide와 boundary 정보를 heightfield가 읽을 수 있는 field로 바꾸는 중간 layer다.
- meso feature는 macro guide와 hydrology constraint를 읽은 뒤 Perlin보다 큰 국소 지형 deformation plan을 만든다.
- Perlin micro relief는 마지막 표면 디테일이며 macro ownership을 뒤집지 않는다.
- material, water, vegetation은 plan으로 만든 뒤 마지막 voxel fill에서 함께 반영한다.

---

## Runtime Cache Contract

Delaunay/Voronoi graph와 그 위의 macro annotation은 청크가 요청될 때마다 즉석에서 새로 만들면
안 된다. chunk는 지형 정체성의 소유자가 아니라 저장/출력 window이므로, 실시간 chunk generation
path는 이미 계산된 world-owned generation cache를 읽어 column/voxel 결과만 합성해야 한다.

런타임 생성은 아래 계층을 따른다.

```text
graph region cache
-> macro map cache
-> hydrology / boundary / heightfield cache
-> chunk generation samples column/window data
-> voxel fill writes ChunkData
```

### Graph Region Cache

graph region cache는 seed, generator version, graph region coordinate, graph config를 key로 하는
world-owned cache다. Delaunay triangulation과 Voronoi corner/edge assembly는 graph region cache
miss에서만 worker thread가 수행한다.

청크 생성 path의 불변식:

- chunk 하나를 채우기 위해 Delaunay triangulation을 반복 실행하지 않는다.
- 여러 chunk가 같은 graph region 또는 overlapping padded graph patch를 공유한다.
- padded graph patch의 authoritative interior만 downstream stage가 신뢰한다.
- hull/open Voronoi edge와 guard 영역은 cache 내부 안정성 장치이며, chunk-visible terrain identity가 아니다.

### Stage Cache Chain

각 stage cache는 이전 stage output을 읽어 deterministic annotation을 만든다.

- `GraphRegionCache`: site/corner/edge topology와 base graph field
- `MacroMapCache`: continent/ocean/island ownership, signed macro elevation, ridge/fault/coast guide
- `HydrologyCache`: selected river chain, watershed, lake/sink/outlet resolution
- `BoundaryCache`: graph/macro/hydrology output을 읽어 만든 selected visible edge의 noisy realization
- `HeightfieldCache`: chunk column sampling이 읽을 height/water/constraint field

초기 구현에서는 이 캐시들이 하나의 넓은 graph patch value로 묶여 있을 수 있다. 그래도 public
계약은 “chunk fill이 graph/macro/hydrology를 생성하지 않고 읽는다”는 방향을 유지해야 한다.

### Job Boundary

`jobs`는 cache miss를 worker thread에서 실행할 수 있지만, stage order나 terrain meaning을 소유하지
않는다. 어떤 cache가 필요하고, 어떤 key와 padding으로 생성해야 하는지는 `world::generation`의
문서와 타입 계약이 소유한다. app/ECS는 chunk visibility와 요청 우선순위를 정할 수 있지만, graph
region cache의 내부 의미를 직접 결정하지 않는다.

### Performance Budget

실시간 chunk generation 기준에서 목표는 아래와 같다.

- chunk fill hot path: Delaunay triangulation 0회
- graph region cache miss: worker에서 Delaunay triangulation 1회
- 같은 graph/macro region을 참조하는 chunk들은 cached stage output 공유
- preview처럼 큰 world window를 한 번에 triangulate하는 경로는 diagnostic binary에 한정
- cache eviction은 메모리 예산을 보되, eviction 후 재생성해도 같은 seed/config/key에서 같은 결과를 내야 함

이 계약을 어기면 플레이어 이동 중 같은 graph patch를 chunk마다 반복 계산해 latency spike가 생길 수
있다.

---

## 불변식

1. chunk coordinate는 output window만 고르며 macro terrain identity를 정의하지 않는다.
2. graph와 hydrology guide는 base heightfield가 voxel로 확정되기 전에 계산되어야 한다.
3. sea-level contract는 `WorldMeta.generator_version`이 의도적으로 바꾸기 전까지 world-space `y = 0`을 유지한다.
4. stage output은 preview와 test가 같은 deterministic 입력으로 재현할 수 있어야 한다.
5. 모든 stage는 독립 topdown preview 대상이어야 한다.
6. chunk fill hot path는 graph triangulation이나 macro ownership resolve를 반복 수행하지 않고,
   world-owned generation cache를 읽어야 한다.

---

## 현재 구현 상태

- 현재는 v2 pipeline compile-time scaffold 단계다.
- legacy generation entrypoint는 migration 동안 `world::generation`을 통해 re-export된다.
- stage 7 boundary realization은 `world::generation::boundary`에 구현되어 있으며, 현재는 graph patch,
  macro map, hydrology graph를 입력으로 `BoundaryCache { curves, stats }`를 생성한다. chunk fill은 이
  curve를 직접 만들지 않고 boundary cache를 읽는 방향을 유지한다.
