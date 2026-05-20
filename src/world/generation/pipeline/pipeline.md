# pipeline

## 역할

`pipeline`은 graph-first world generation의 stage order와 column synthesis scaffold를 정의한다.

이 모듈은 새 파이프라인이 legacy generator를 대체하기 전까지 compile-time stage contract를
제공한다. 실제 stage 구현은 `graph`, `macro_map`, `hydrology`, `river_plan`, `boundary`, `field`,
`meso_feature`, `macro_field`, `pixelize`, `heightfield`, `surface_plan`, `voxel`, `preview` 문서와 구현으로 분산된다.

---

## 책임

- graph-first generation stage 이름 정의
- initial generation config surface 정의
- chunk voxelization이 나중에 소비할 pixelize/column synthesis request/result shape 정의
- stage 순서가 macro guide, hydrology, pixelize, heightfield, voxel fill 순서를 어기지 않도록 고정

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
7. river realization / river plan
8. final cell context / climate / hydration / biome resolve
9. noisy boundary realization
10. meso feature planning
11. macro field rasterization
12. chunk pixelize
13. heightfield / voxel-column realization
14. surface plan
15. vegetation plan
16. voxel fill

pipeline은 더 세분화될 수 있지만, 반드시 아래 대원칙을 지켜야 한다.

- graph construction이 먼저다.
- base climate와 elevation seed는 graph 단계에서 시작하지만, final temperature/hydration/biome influence는 hydrology 이후,
  boundary와 macro_field 이전에 final cell context로 resolve한다.
- base `continentality`와 `elevation_seed`는 macro_map ownership/elevation resolve의 source of truth다.
- macro_map은 독자적인 continent/island noise source를 만들지 않고 graph base field를 해석한다.
- continent/ocean/island ownership과 macro elevation은 Perlin보다 먼저다.
- ridge/fault edge guide와 coast edge guide는 hydrology보다 먼저다. mountainness/rugged context는 이 guide를 고르는 입력이다.
- hydrology는 final heightfield와 voxel fill보다 먼저다.
- hydrology는 potential river guide가 아니라 selected river chain, flow accumulation, lake/sink/outlet resolution을 만든다.
- river plan은 hydrology 이후에 selected river chain을 terrain morphology parameter로 번역한다.
  hydrology가 “어디로 흐르는가”를 결정한다면, river plan은 “이 reach가 얼마나 넓고 깊은 valley와
  bed를 가져야 하는가”를 결정한다.
- final cell context는 macro signed elevation, water proximity, rain shadow, selected hydrology role을 읽어
  final temperature, hydration, biome influence, optional dominant biome id를 제공한다.
- biome은 macro_field보다 먼저 resolve된다. macro_field는 biome을 새로 결정하지 않고 final cell
  context를 raster/cache 가능한 sample channel이나 downstream hint로 보존한다.
- noisy boundary는 모든 Voronoi edge의 canonical geometry layer이며 raw graph topology를 대체하지 않는다.
  river는 별도 noisy curve를 만들지 않고 selected edge id path가 이 canonical geometry를 따른다.
- meso feature planning은 noisy boundary 이후, macro field 이전에 실행한다. 이 단계는 macro보다 작고
  Perlin보다 큰 hill/knob/ravine/terrace 같은 deterministic feature object table을 만들며, selected
  hydrology와 macro ownership을 뒤집지 않는다. primary drainage를 바꾸는 feature는 ordinary meso가
  아니라 macro/hydrology guide로 승격해야 한다.
- macro field rasterization은 graph/macro/hydrology/river-plan/final-cell-context/boundary/meso-feature 결과를 pixelize와 downstream heightfield가
  빠르게 읽을 수 있는 graph-derived signed distance / influence field cache로 굽는 중간 layer다.
  이 단계는 새 noise source가 아니며, source of truth는 앞 단계의 vector/graph annotation에 남아 있다.
  macro_field는 `MesoFeaturePlan`의 raise/carve/flatten/roughness contribution을 sample channel과
  `combined_macro_height`에 bake한다.
- chunk pixelize는 stage 11 `MacroFieldTile`만 소비해 chunk boundary에 정렬된
  `1 world block = 1 pixel = 1 voxel column` output을 만든다. 이 단계는 graph topology, hydrology,
  river plan, noisy boundary, meso feature geometry를 다시 해석하지 않고 source `MacroFieldSample`의 channel을 column
  좌표계와 integer surface/water hint로 옮긴다.
- heightfield / voxel-column realization은 pixelized column output을 downstream input으로 소비한다.
  새 path에서 heightfield는 first chunk-aligned pixel resolve를 다시 수행하거나 `MacroFieldTile`을
  직접 resample하지 않는다. heightfield는 meso feature geometry를 다시 탐색하지 않고
  macro_field/pixelize가 보존한 meso-baked column value와 optional Perlin micro relief를 소비한다.
- material, water, vegetation은 plan으로 만든 뒤 마지막 voxel fill에서 함께 반영한다. 현재 launch
  저장 slice에서는 surface/material/vegetation plan을 stub으로 두고, `PixelizedColumn.surface_y`와
  `water_y`만 읽어 비물 지형은 `grass`, 물은 `water`로 채운다.

---

## Runtime Cache Contract

Delaunay/Voronoi graph와 그 위의 macro annotation은 청크가 요청될 때마다 즉석에서 새로 만들면
안 된다. chunk는 지형 정체성의 소유자가 아니라 저장/출력 window이므로, 실시간 chunk generation
path는 이미 계산된 world-owned generation cache를 읽어 column/voxel 결과만 합성해야 한다.

런타임 생성은 아래 계층을 따른다.

```text
graph region cache
-> macro map cache
-> hydrology cache
-> river plan cache
-> final cell context cache
-> boundary cache
-> meso feature cache
-> macro field tile cache
-> pixelized chunk area cache
-> heightfield / voxel-column realization cache
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
- `RiverPlanCache`: selected river chain의 downstream progress, reach type, broad valley와 narrow
  river bed parameter
- `FinalCellContextCache`: final temperature, hydration, hydrology role, water proximity, rain shadow,
  biome influence, optional dominant biome id
- `BoundaryCache`: 모든 graph edge id에 대한 canonical noisy polyline/spline
- `MesoFeatureCache`: macro보다 작고 Perlin보다 큰 feature plan table, protected mask, height/material hint
- `MacroFieldTileCache`: macro elevation, coast/lake/ocean/dry basin mask, ridge/fault influence,
  river valley field, final cell context/biome influence, combined macro height 같은 graph-derived raster field
- `PixelizedChunkAreaCache`: chunk-aligned `1 world block = 1 pixel = 1 voxel column` resolved columns,
  integer `surface_y`, optional integer `water_y`, terrain kind hint, source macro masks
- `HeightfieldCache`: pixelized column output을 읽어 downstream voxel-column/surface path가 소비할
  surface height, water level, terrain kind hint field

초기 구현에서는 이 캐시들이 하나의 넓은 graph patch value로 묶여 있을 수 있다. 그래도 public
계약은 “chunk fill이 graph/macro/hydrology를 생성하지 않고 읽는다”는 방향을 유지해야 한다.

`MacroFieldTileCache`는 chunk fill hot path의 graph query 반복을 막기 위한 cache canvas다. chunk
pixelize stage는 nearest graph edge, noisy curve distance, lake containment, ridge envelope,
river plan guide distance를 직접 반복 계산하지 않고, macro field tile의 sample 값을 읽는다. tile cache miss는
worker에서 graph/macro/hydrology/river-plan/final-cell-context/boundary cache를 입력으로 rasterize한다.

ridge/coast/river influence는 tile cache miss에서 curve source를 rasterize하고 distance/influence
field로 전파한다. 따라서 chunk fill이나 preview render loop는 selected river/ridge/coast curve의
polyline distance를 sample마다 반복하지 않는다. ownership과 macro elevation의 noisy-boundary
side/blend 판정은 launch 단계에서 정확도 우선으로 per-sample query가 남아 있을 수 있지만, 이것도
chunk hot path가 아니라 macro field cache miss에서만 수행되어야 한다.

macro field tile의 기본 channel은 아래를 포함해야 한다.

- macro elevation: signed macro elevation을 noisy boundary 기준으로 연속 샘플링한 큰 지형 높이
- coast/lake/ocean/dry basin mask: water ownership, lake flatten, shoreline diagnostics/downstream policy가 읽는 mask/distance
- ridge/fault influence: ridge/fault guide edge의 canonical noisy curve 주변 envelope
- river valley: river plan의 broad valley parameter를 rasterize한 distance/flow/carve strength와,
  heightfield/water가 읽을 narrow bed hint
- meso contribution: feature plan에서 온 raise/carve/flatten/roughness/material hint
- combined macro height: macro elevation, ridge raise, broad river valley, lake flatten, meso contribution을 합성한 pre-Perlin height

pixelized chunk area cache는 macro field 이후에 생성된다. launch vertical slice에서는 이 cache가
`combined_macro_height`를 block-space column으로 매핑하고, ocean/lake mask에서 water level hint를
만들며, ridge/river/dry basin/meso channel을 terrain kind hint로 보존한다. heightfield rewrite는 이
pixelized column output을 소비해 optional Perlin detail, surface/water safety, voxel-column realization을
이어간다.

launch graph-first save path는 `PixelizedChunkAreaCache` 이후에 얇은 `GraphFirstVoxelPlan`을 만든다.
이 plan은 surface/material/vegetation 정책을 확장하지 않고 `surface_y`/`water_y`만 보존한다. 같은
plan을 여러 y chunk에 재사용하므로 vertical chunk stack은 같은 x/z column 결과를 공유한다.

### Job Boundary

`jobs`는 cache miss를 worker thread에서 실행할 수 있지만, stage order나 terrain meaning을 소유하지
않는다. 어떤 cache가 필요하고, 어떤 key와 padding으로 생성해야 하는지는 `world::generation`의
문서와 타입 계약이 소유한다. app/ECS는 chunk visibility와 요청 우선순위를 정할 수 있지만, graph
region cache의 내부 의미를 직접 결정하지 않는다.

### Performance Budget

실시간 chunk generation 기준에서 목표는 아래와 같다.

- chunk fill hot path: Delaunay triangulation 0회
- chunk fill hot path: nearest graph edge/curve search 0회 또는 bounded cached lookup
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
7. macro field tile은 noise source가 아니라 graph-derived cache이며, meso feature는 이 cache에
   bake되고 Perlin micro relief는 이 cache 이후에만 합성된다.
8. chunk pixelize는 stage 11 `MacroFieldTile`을 stage 13 heightfield/voxel-column path가 읽을
   chunk-aligned column cache로 바꾸는 유일한 first pixel resolve 단계다.

---

## 현재 구현 상태

- 현재는 v2 pipeline vertical slice 단계다.
- legacy generation entrypoint는 migration 동안 `world::generation`을 통해 re-export된다.
- graph-first `pixelize`와 launch `voxel` fill이 연결되어 `world_create`가 bounded created-world dump를
  저장할 수 있다.
- surface/material/vegetation은 아직 stub이며, 최종 block palette policy는 들어오지 않았다.
