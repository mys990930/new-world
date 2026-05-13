# river_plan

## 역할

`river_plan`은 hydrology가 선택한 river graph를 terrain generation이 쓰기 쉬운 형태로
번역하는 중간 단계다.

hydrology는 물이 어디로 흐르는지, 어떤 edge가 selected river인지, lake/sink/outlet이 어디인지
결정한다. `river_plan`은 그 결과를 읽어 river reach의 성격과 morphology parameter를 정한다.
즉 이 단계는 새 물리 시뮬레이션이나 새 river path 생성기가 아니라, selected hydrology result를
macro field와 heightfield가 읽을 수 있는 plan으로 바꾸는 realization layer다.

---

## 책임

- selected river segment를 river chain/reach 단위로 묶는다.
- downstream progress, flow/order, terminal role을 기준으로 reach type을 분류한다.
- reach별 broad valley와 narrow bed parameter를 결정한다.
- macro field가 읽을 broad valley guide를 제공한다.
- heightfield/water stage가 읽을 river bed / water hint parameter를 제공한다.
- branch, confluence, lake inlet/outlet, sink/outlet terminal에서 plan이 끊기거나 합쳐지는 규칙을 정의한다.

---

## 비책임

- downhill routing, watershed, lake/sink/outlet 선택
- selected river segment 자체의 생성 또는 제거
- canonical noisy boundary 생성
- macro elevation, coast/lake/ocean/dry basin ownership resolve
- final water surface solve
- final voxel channel carve
- material, vegetation, biome 선택

---

## 입력과 출력

입력:

```text
VoronoiGraphPatch
GraphMacroMap
GraphHydrologyGraph
BoundaryCache
```

출력:

```text
RiverPlan {
    chains,
    reaches,
    per_segment_parameters,
}

RiverReach {
    chain_id,
    segment_ids,
    reach_type,
    downstream_start,
    downstream_end,
    display_flow,
    stream_order_hint,
    broad_valley_width_blocks,
    broad_valley_depth,
    bed_width_blocks,
    bed_depth,
    bank_transition_width_blocks,
    floodplain_width_blocks,
}
```

현재 compile bridge 구현은 위 이름을 공개 API로 제공한다. 이 구현은 rollback 이후 downstream
stage가 기대하는 계약을 복구하기 위한 얇은 realization layer이며, 복잡한 meander/cusp 보정이나
full chain smoothing은 다시 넣지 않는다.

---

## Reach Type

river plan은 모든 selected river를 하나의 `river_valley_strength`로 취급하지 않는다. 최소한 아래
범주를 구분한다.

| reach type | 입력 감각 | broad valley | river bed |
|---|---|---|---|
| `Headwater` | 낮은 flow/order | 매우 좁고 급함 | 얕고 매우 좁음 |
| `Upper` | 작은 지류 | 좁고 비교적 급함 | 얕고 좁음 |
| `Middle` | 중간 flow | 중간 폭, 완만한 shoulder | 어느 정도 평평 |
| `Lower` | 큰 flow | 넓고 완만한 valley | 깊고 넓은 flat bed |
| `Trunk` | 주 하천 | floodplain 성격의 넓은 valley | 넓고 평평한 강바닥 |
| `LakeInlet` | lake capacity 제한을 받은 feeder | 짧고 보수적인 valley | lake boundary 직전에서 종료 |
| `LakeOutlet` | lake에서 다시 시작하는 outflow | 낮은 outlet 쪽으로 완만히 시작 | lake와 떨어진 land-side endpoint에서 시작 |

분류 기준은 deterministic heuristic이면 충분하다. launch 기준 후보:

- selected/display flow accumulation
- raw flow accumulation
- stream order 또는 upstream branch count
- downstream progress
- lake/sink/ocean terminal role
- local macro slope와 coast/lake distance

하류일수록 broad valley width, bed width, bed depth, floodplain width가 커져야 한다. 상류와 하류가
같은 폭과 깊이를 가지면 회귀다.

---

## Broad Valley와 River Bed 분리

`river_plan`의 가장 중요한 계약은 broad valley와 river bed를 분리하는 것이다.

```text
macro_field:
  broad_valley_width/depth를 읽어 큰 지형 계곡만 완만하게 낮춘다.

heightfield/water:
  bed_width/depth와 water hint를 읽어 좁은 실제 강바닥/수면을 snap한다.
```

macro field의 `combined_macro_height`는 강바닥을 깊게 파서 river shape를 완성하려고 하면 안 된다.
이 단계에서 좁고 강한 carve가 보이면 bend에서 capsule/blob artifact가 커진다. macro field는 넓고
약한 valley morphology를 보여주고, 실제 river bed와 visible water는 heightfield/water/surface 단계에서
별도 hint로 확정한다.

---

## Geometry Policy

hydrology selected edge path는 river topology다. `river_plan`은 이 topology를 버리지 않는다.

- selected river segment는 여전히 Voronoi edge id와 downstream corner logic을 따른다.
- canonical noisy boundary curve는 river가 지나갈 수 있는 graph corridor를 제공한다.
- river plan은 graph corridor 안에서 reach별 width/depth/profile을 정하지만, 새 random river network를 만들지 않는다.
- segment와 segment 사이의 visible artifact는 geometry smoothing 하나로 해결하지 않는다.
  broad valley와 narrow bed를 분리하고, bed 단계에서 필요한 경우에만 local join 처리를 한다.

launch 구현에서는 복잡한 offset polygon이나 full hydraulic geometry를 요구하지 않는다. 먼저 reach별
parameter와 downstream-progress 기반 profile을 만들고, macro field가 broad valley만 쓰도록 바꾸는
것이 우선이다.

---

## 처리 순서

1. `GraphHydrologyGraph.segments`에서 selected river segment를 읽는다.
2. downstream corner 관계를 따라 chain을 만든다.
   - branch/confluence는 명시적으로 표시한다.
   - lake inlet에서 끝난 chain과 lake outlet에서 시작하는 chain은 별도 chain이다.
3. 각 chain에 downstream progress를 누적한다.
4. 각 segment/reach의 display flow, raw flow, terminal role을 읽는다.
5. reach type을 분류한다.
6. reach type과 flow/order를 기반으로 valley/bed/floodplain parameter를 계산한다.
7. macro field와 heightfield가 사용할 plan table을 만든다.

---

## Runtime Cache

`RiverPlan`은 hydrology cache 이후, final cell context와 boundary/macro field 이전에 준비되는
world-owned cache다.

```text
graph region cache
-> macro map cache
-> hydrology cache
-> river plan cache
-> final cell context cache
-> boundary cache
-> macro field tile cache
-> heightfield / water surface cache
-> voxel fill
```

chunk fill hot path는 river chain을 다시 만들거나 selected segment를 다시 분류하지 않는다.

---

## Preview

초기에는 별도 `river_plan_preview`가 없어도 된다. 대신 기존 preview가 아래 정보를 metadata 또는
diagnostic overlay로 볼 수 있어야 한다.

- reach type별 색상 또는 count
- broad valley width/depth range
- bed width/depth range
- lake inlet/outlet reach count
- headwater/upper/middle/lower/trunk count

사용자가 명시적으로 요청하기 전까지 preview 이미지 생성을 자동으로 추가하지 않는다.

---

## 불변식

1. `river_plan`은 selected river segment를 새로 선택하거나 제거하지 않는다.
2. selected river topology는 hydrology의 source of truth다.
3. reach type과 width/depth parameter는 deterministic해야 한다.
4. downstream으로 갈수록 display flow가 커지는 trunk에서는 broad valley와 bed width가 커져야 한다.
5. lake terminal/inlet reach는 lake capacity cap을 존중해야 하며 ocean outlet trunk처럼 넓어지면 안 된다.
6. macro field는 broad valley를 주로 읽고, narrow bed를 combined height에 과하게 직접 반영하면 안 된다.
7. heightfield/water는 river bed/water hint를 읽을 수 있지만, hydrology routing을 다시 풀면 안 된다.
8. `river_plan`은 final material이나 voxel fill을 직접 결정하지 않는다.

---

## 현재 구현 상태

- `src/world/generation/river_plan/mod.rs`가 `RiverPlanConfig`, `RiverPlan`, `RiverChain`,
  `RiverReach`, `RiverSegmentPlan`, `RiverPlanStats`, `build_river_plan`을 제공한다.
- 현재 구현은 hydrology selected segment를 보존하고, 각 segment를 하나의 lightweight chain/reach로
  노출한다. topology 선택의 source of truth는 여전히 `GraphHydrologyGraph.segments`다.
- `build_river_plan(&VoronoiGraphPatch, &GraphMacroMap, &GraphHydrologyGraph, RiverPlanConfig)`는
  selected segment의 edge, display/raw flow, downstream progress, local slope를 읽어 reach type과
  broad valley / bed / bank hint 값을 만든다.
- 이 구현은 `af41834` rollback 이후 compile contract를 복구하기 위한 최소 bridge다. 기존의
  aggressive morphology smoothing, mouth fan, cusp 보정, per-chain hydraulic post-pass는 되살리지
  않았다.
- `macro_field`는 다시 `RiverPlan`을 입력으로 받아 selected edge의 canonical noisy boundary curve와
  plan의 display flow를 rasterize한다. narrow bed hint는 downstream heightfield가 읽을 diagnostic
  hint로 보존한다.
