# graph

## 역할

`graph`는 graph-first world generation의 Voronoi-style macro graph 계약을 소유한다.

이 모듈은 site, corner, edge, graph region을 정의한다. 이 구조는 최종 지형 모양이 아니라
월드의 거시 의미와 제약을 담는 내부 graph다.

---

## 책임

- world-space block 좌표를 graph cache region으로 매핑
- graph region 단위 deterministic site 후보 생성 계약
- site, corner, edge id와 patch container 정의
- center graph와 corner graph의 dual topology 계약
- site/corner/edge가 다른 feature layer에서 annotation될 수 있는 id surface 제공
- `WorldMeta.seed`, `generator_version`, `GraphRegionCoord` 기반 determinism 유지

---

## 비책임

- final biome selection
- river path solving
- noisy boundary curve 생성
- noise synthesis 또는 final heightfield 출력
- block placement
- live chunk storage mutation

---

## Macro Voronoi Graph

먼저 world-space에 deterministic Voronoi-style macro graph를 만든다.

권장 방식:

- graph region 단위로 site 후보를 생성한다.
- 순수 random point는 clumping이 심하므로 Poisson Disc 또는 jittered grid + 제한된 relaxation을 우선 검토한다.
- Lloyd relaxation은 polygon 크기를 고르게 만드는 데 유용하지만, 너무 많이 적용하면 grid처럼 규칙적이 된다.
- 무한 월드에서는 region-local relaxation이 경계 불안정을 만들 수 있으므로, padding을 포함한 patch 단위 안정성 테스트가 필요하다.
- 최종 graph가 꼭 수학적으로 완전한 Voronoi일 필요는 없다. corner 간격과 polygon shape가 더 균질한 barycentric dual mesh 계열도 후보가 될 수 있다.

이 단계의 출력은 site, corner, edge, adjacency를 포함한 graph patch다.

현재 구현은 launch 단계의 안정성을 우선해 고정 density jittered grid 기반의 Voronoi-style
barycentric dual patch를 만든다. 완전한 Delaunay/Voronoi 계산은 아니지만, 전역 site lattice를
seed와 generator version으로 jitter하고, 2x2 site 평균점을 corner로 삼으며, 인접 site 쌍을 edge로
연결한다. 따라서 site, corner, edge topology가 실제로 존재하고, 같은 전역 lattice 좌표는 어떤
patch 요청에서 생성하더라도 같은 id와 위치를 갖는다.

공개 생성 API는 graph leaf가 소유한다.

```rust
GraphBaseFields {
    temperature,
    hydration,
    continentality,
    elevation_seed,
}

GraphBaseFieldConfig {
    smoothing_passes,
    self_weight,
}

VoronoiGraphConfig {
    seed,
    generator_version,
    region_size_blocks,
    site_spacing_blocks,
    padding_regions,
}

VoronoiGraphPatchRequest::new(config, center_world_x, center_world_z)
generate_voronoi_graph_patch(request) -> VoronoiGraphPatch
apply_base_graph_fields(patch, GraphBaseFieldConfig::default())
```

`center_world_x/z`는 Euclidean division으로 중심 graph region을 고른다. `owner_regions`는 요청의
중심 region을 나타내며, 실제 site/corner/edge 후보는 `padding_regions`만큼 확장한 주변 region과
추가 site-cell guard에서 생성한다. 이 guard는 patch 바깥 boundary edge를 조립하기 위한 내부 계산
범위다. base field smoothing은 이웃 site를 여러 pass 읽으므로, 현재 구현은 topology guard에
기본 smoothing pass 수만큼 site-cell guard를 더해 overlap 영역의 smoothed field가 요청 중심에
따라 달라지지 않게 한다.

생성 단계는 rayon으로 site와 corner, edge 후보를 병렬 계산한다. 병렬 수집 뒤에는 id 기준 정렬과
dedup을 수행하므로 thread scheduling은 결과 순서에 영향을 주지 않는다.

## Base Graph Field Stage

pipeline 2단계는 graph leaf 안에서 명시적인 base field stage로 실행된다.
`generate_voronoi_graph_patch`는 topology를 만든 뒤 `apply_base_graph_fields`를 호출해 site와
corner에 base temperature, hydration, continentality, elevation seed를 채운다.

`GraphBaseFields`는 stage 2의 공용 field 묶음이다.

- `temperature`: 0..1 base climate seed
- `hydration`: 0..1 base humidity/hydration seed
- `continentality`: -1..1 base continent/ocean tendency seed
- `elevation_seed`: -1..1 hydrology 전 macro elevation bias seed

site는 두 값을 함께 가진다.

- `raw_base_fields`: seed와 lattice coordinate에서 직접 나온 raw 값
- `base_fields`: center graph adjacency를 참고해 smoothing된 값

기존 `VoronoiSite.temperature`, `hydration`, `height_bias`, `continentality`는 downstream 호환을
위해 `base_fields`를 복사한 convenience field다. 새 code는 raw/smoothed 구분이 필요하면
`raw_base_fields`와 `base_fields`를 직접 읽는다.

corner도 `raw_base_fields`와 `base_fields`를 가진다. corner 값은 독립 hash가 아니라 surrounding
site 4개의 raw/smoothed base field를 corner와 site position 사이 거리로 가중 평균해 만든다.
`VoronoiCorner.elevation`은 이 단계에서는 hydrology solve 결과가 아니라 `base_fields.elevation_seed`
를 복사한 base elevation bias다. downhill, water accumulation, lake/sink/outlet 처리는 이후
hydrology 단계가 별도 layer에서 소유한다.

---

## Site / Corner / Edge

site는 지역의 중심 의미를 가진다.

- dominant biome owner 후보
- temperature seed
- hydration seed
- elevation bias
- continentality
- ruggedness
- local naming / gameplay region owner

corner는 지형 흐름의 계산점이다.

- base elevation seed / bias
- downhill direction
- water accumulation
- lake/sink/outlet state
- watershed id

edge는 두 site와 두 corner 사이의 관계다.

- biome transition 후보
- coast 후보
- river 후보
- fault/cliff/plateau 후보
- noisy boundary realization seed

---

## Dual Graph Representation

world graph는 두 개의 연결된 graph를 가진다.

- center graph: polygon site를 node로 하고, 인접 polygon 사이를 edge로 둔다.
- corner graph: polygon corner를 node로 하고, polygon boundary segment를 edge로 둔다.

center graph는 지역 인접성, pathfinding, named area grouping, biome owner propagation,
settlement/quest/strategic value analysis에 유용하다.

corner graph는 실제 polygon boundary, coast line, river path, downhill routing, watershed,
lake outlet, noisy edge guard geometry에 유용하다.

하나의 Voronoi edge는 보통 `두 site + 두 corner`를 함께 알아야 한다. 이 구조를 유지해야 강,
해안선, 단층, 절벽 같은 edge 기반 feature가 같은 graph topology를 공유하면서도 서로 다른
현실화 규칙을 가질 수 있다.

---

## Module Annotation Model

graph core에 모든 feature field를 직접 박아 넣으면 빠르게 비대해진다. core graph는 index/id를
제공하고, feature module은 외부 table로 annotate한다.

권장 방향:

- `VoronoiGraphPatch`는 site/corner/edge id와 topology를 소유한다.
- hydrology는 `edge_id -> river segment`, `corner_id -> drainage node` 같은 별도 layer를 가진다.
- biome은 `site_id -> biome owner`, `column -> blended influence` 별도 layer를 가진다.
- boundary realization은 `edge_id -> noisy curve` cache를 가진다.
- faults/lava/ecology 같은 feature는 core graph에 직접 의존하지 않고 id 기반 layer로 붙는다.

이 방식은 world 내부 의존성을 줄인다. `graph`는 `hydrology`를 몰라도 되고, `hydrology`는 필요한
graph id와 샘플만 읽는다.

---

## Variable Density

모든 지역에 같은 graph density를 쓸 필요는 없다.

후보 정책:

- 보통 야생 지역은 coarse graph
- coast, river confluence, settlement 후보, dungeon 주변은 finer graph
- 큰 ocean이나 평야는 낮은 density
- 산맥, 계곡, 습지, 경계 지형은 높은 density

다만 variable density는 무한 월드에서 어려운 문제다. site spacing이 region 경계에서 바뀌면
artifact가 생길 수 있다. 따라서 launch 단계에서는 고정 density를 먼저 쓰고, graph schema만
variable density를 막지 않게 열어둔다.

---

## Terrain Analysis

polygon graph는 빠른 terrain analysis에 유용하다.

분석 후보:

- shortest path와 euclidean distance 차이
- chokepoint
- coast 접근성
- mountain pass
- river crossing
- frequently-used path region
- isolated valley
- strategic settlement candidate

이 분석은 `ecs`가 아니라 `world` 또는 향후 world-owned analysis layer가 소유해야 한다. ECS는
그 결과를 gameplay 의미로 해석할 수 있지만, 원본 지형 graph와 접근성 계산은 world 데이터에
가깝다.

---

## 불변식

1. graph region은 cache와 ownership 단위일 뿐 visible terrain unit이 아니다.
2. Voronoi site, corner, edge는 macro semantic guide이며 최종 block-space shape가 아니다.
3. neighboring graph region은 충분한 padding으로 생성되어 ownership 경계를 넘는 site와 edge가 안정적이어야 한다.
4. negative world coordinate는 Euclidean division을 사용해 모든 사분면에서 region ownership이 안정적이어야 한다.
5. graph core는 feature-specific state를 직접 끌어안지 않고 id 기반 annotation layer를 허용해야 한다.
6. 같은 `VoronoiGraphConfig`와 같은 전역 lattice 좌표에서 생성된 site/corner/edge는 요청 중심이 달라도 같은 결과를 가져야 한다.
7. seed와 generator version은 site jitter, base field seed, edge seed에 반영되어야 한다.
8. 병렬 생성은 최종 정렬/dedup 이후 deterministic해야 한다.

---

## 현재 구현 상태

- data contract와 coordinate helper가 있으며, seed 기반 deterministic padded Voronoi-style patch 생성이 구현되어 있다.
- 구현된 patch 생성은 고정 density jittered grid와 barycentric dual topology를 사용한다.
- pipeline 2단계 base graph field가 구현되어 있으며, site raw seed와 smoothed base field,
  corner 주변 site 기반 base field/elevation seed를 제공한다.
- 아직 구현되지 않은 것: Lloyd relaxation, 실제 Delaunay/Voronoi construction, variable density, hydrology routing, noisy boundary realization.
