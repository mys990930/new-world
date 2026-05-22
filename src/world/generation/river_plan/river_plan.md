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
- branch, pruned upstream drainage join, lake inlet/outlet, sink/outlet terminal에서 plan이 끊기거나
  합쳐지는 규칙을 정의한다.

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
    segment_endpoints,
    stats,
}

RiverReach {
    chain_id,
    segment_ids,
    reach_type,
    downstream_start,
    downstream_end,
    display_flow,
    upstream_area,
    tributary_flow,
    discharge_q,
    morphology_discharge_q,
    hydraulic_width_coefficient,
    hydraulic_depth_coefficient,
    velocity,
    stream_order_hint,
    broad_valley_width_blocks,
    broad_valley_depth,
    bed_width_blocks,
    bed_depth,
    bank_transition_width_blocks,
    floodplain_width_blocks,
}
```

현재 구현은 `RiverPlan`, `RiverChain`, `RiverReach`, `RiverSegmentPlan`으로 이 의미를 직접
노출한다. `build_river_plan(&VoronoiGraphPatch, &GraphMacroMap, &GraphHydrologyGraph,
RiverPlanConfig)`는 hydrology의 selected segment set을 보존하며, lake edge selected segment 같은
위반은 제거하지 않고 `RiverPlanStats`에 기록한다.

`RiverSegmentPlan`은 reach morphology 값과 hydrology-derived flow diagnostics를 노출한다.
segment별 downstream endpoint 정보는 `RiverPlan.segment_endpoints`와 `RiverPlan::endpoints(id)`에서
별도 table로 제공한다. 이 endpoint는 hydrology segment의 `from -> to` 방향을 그대로 따른다.

현재 구현은 selected hydrology topology를 다시 고르지 않지만, selected adjacency를 읽어 deterministic
chain/reach table로 현실화한다. 각 selected segment는 hydrology의 `from -> to`, edge id,
canonical river-system Q인 `flow_accumulation`, `raw_flow_accumulation`, `downstream_progress`,
`local_slope`를 보존한다. 그 위에 chain-local downstream pass로 `upstream_area`, `tributary_flow`,
`discharge_q`, `morphology_discharge_q`, coefficient, velocity field를 채운다. 이 pass는 selected
segment를 추가/삭제하지 않으며, ordinary chain 안에서는 display discharge와 morphology scale이
downstream으로 줄어들지 않게 한다. lake inlet/outlet은 canonical Q를 reset하지 않고, lake reach type의
local morphology bounds로만 ocean trunk보다 보수적인 shape를 만든다.
hydrology는 이제 final selected graph 단계에서 ocean/coast reachability, selected/display discharge
monotonicity, lake inlet/outlet Q propagation을 먼저 보장한다. `river_plan`은 이 invariant를 소비하되,
기존처럼 defensive chain-local ledger로 downstream morphology scale이 줄어들지 않게 유지한다.

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
| `LakeInlet` | lake transition feeder | canonical Q를 보존하되 짧고 보수적인 valley | lake boundary 직전에서 종료 |
| `LakeOutlet` | lake에서 다시 시작하는 outflow | canonical Q를 이어받되 낮은 outlet 쪽으로 완만히 시작 | lake와 떨어진 land-side endpoint에서 시작 |

분류 기준은 deterministic heuristic이면 충분하다. launch 기준 후보:

- canonical selected/display flow accumulation
- raw flow accumulation
- stream order 또는 upstream branch count
- downstream progress
- lake/sink/ocean terminal role
- local macro slope와 coast/lake distance

하류일수록 broad valley width, bed width, bed depth, floodplain width가 커져야 한다. 상류와 하류가
같은 폭과 깊이를 가지면 회귀다.
hydrology role의 `Trunk`/`Floodplain` label은 topology/diagnostic hint일 뿐, river_plan에서는 낮은 Q
segment를 trunk 단면으로 강제 승격하지 않는다. reach scale은 canonical display Q와 chain progress를
우선하며, 더 큰 Q가 더 작은 morphology로 줄어드는 일만 막는다.

launch scale은 1 block = 0.5m 감각을 기준으로 한다. 상류/중류는 preview에서 과대하게 읽히지 않도록
하류보다 훨씬 좁게 잡는다. 목표 감각은 middle bed/broad valley가 lower의 대략 절반 안팎, upper가
middle의 대략 절반 안팎이다. downstream trunk water width는 raster 단계에서 cell 크기를 다시 읽어
동적으로 정하지 않고, river_plan의 absolute block-scale target을 따른다. launch 기본값은 typical
Voronoi cell width 약 `200` blocks에서 유도한 `downstream_water_width_blocks = 200`이며, trunk Q에
도달한 selected river의 water/bed width가 이 절대 폭 주변으로 수렴해야 한다.

- headwater: bed width roughly `1.5..5` blocks, bed depth roughly `0.6..1.8` blocks.
- upper: bed width roughly `5..18` blocks, bed depth roughly `0.8..2.8` blocks.
- middle: bed width roughly `14..72` blocks, bed depth roughly `1.2..4.8` blocks.
- lower: bed width generally stays in the `55..170` block range; bed depth roughly
  `3..14` blocks.
- trunk: bed/water width generally trends around the `200` block absolute target, with deterministic
  reach variation in roughly the `120..260` block range; bed depth roughly `4..22` blocks.
- lake inlet/outlet은 canonical Q를 유지하되 lake reach type의 local shape bounds 때문에 ocean trunk처럼 과하게 커지지 않는다.

---

## Broad Valley와 River Bed 분리

`river_plan`의 가장 중요한 계약은 broad valley와 river bed를 분리하는 것이다.

```text
macro_field:
  broad_valley_width/depth로 넓은 저지대를 만들고, bed_width/depth로 exposed bed tier와 그 안쪽
  active core trough를 source height에 bake한다.

heightfield/water:
  macro_field가 낮춘 river corridor 중 flow-scaled active-core cutoff를 통과한 lower channel만
  RiverCore로 snap하고 물을 채운다. 그 주변의 낮아진 평평한 tier는 RiverBed로 남겨 수면 주변의
  노출 가능한 bed/gravel/deposition context를 보존한다.
```

macro field의 `combined_macro_height`는 river valley와 active core trough의 지형 맥락을 책임진다.
heightfield/water/surface 단계는 이 source height를 다시 carve하지 않고, 이미 패인 morphology 안에서
exposed bed와 active water core의 ownership만 판정한다. cutoff는 centerline 하나로 줄이는 용도가 아니라
bed tier가 core에 먹히지 않도록 macro_field의 단면 profile을 읽는 용도다.

Low-flow reach의 broad valley는 물/bed depth를 줄이는 방식보다 폭을 좁히는 방식으로 조정한다.
Headwater/upper land carve는 `bed_width_blocks` 주변의 좁은 shoulder로 유지하고, water depth hint와
`bed_depth_blocks`는 heightfield/water 단계가 계속 읽을 수 있게 보존한다.

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

1. `GraphHydrologyGraph.segments`에서 selected river segment를 읽고, hydrology `from -> to` 방향을
   그대로 쓰는 selected adjacency table을 만든다.
2. source, confluence outflow, lake outlet, missing/branch topology break에서 deterministic chain을 시작한다.
   ordinary single-in/single-out vertex는 같은 chain 안에서 downstream으로 이어진다.
3. terminal, lake inlet, sink/outlet, confluence, missing continuation, branch topology break에서 chain을 끝낸다.
4. chain 내부 segment를 reach type이 같은 contiguous run으로 묶어 reach를 만든다. adjacent selected segment가
   같은 flow/role scale이면 더 이상 segment마다 독립 reach가 되지 않는다.
5. 각 segment/reach의 display flow, raw flow, terminal role, chain-local downstream position을 읽어 reach
   type을 분류한다.
6. chain-local ledger를 계산한다.
   - `upstream_area`는 downstream으로 누적된 raw flow floor다.
   - `tributary_flow`는 raw ledger와 selected/display discharge의 차이를 나타내는 diagnostic hint다.
   - `discharge_q`와 `morphology_discharge_q`는 canonical display-flow 기반 terrain scale이다.
   - ordinary chain에서는 downstream으로 줄어들지 않으며, lake inlet/outlet chain boundary에서도
     hydrology가 제공한 canonical system Q를 따른다.
7. reach type, selected/display flow, chain/reach coefficient에서 보수적인 morphology parameter를 계산한다.
   - 이 값은 downstream stage가 읽을 diagnostic/guide이며, macro_field가 복잡한 단면 carve를 직접
     재구성하는 근거가 되어서는 안 된다.
8. macro field와 heightfield가 사용할 plan table을 만든다.
9. downstream renderers가 hydrology 방향을 알 수 있도록 selected segment의 `from_position`,
   `to_position`, `downstream_position` endpoint table을 노출한다.

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
4. ordinary chain의 display discharge와 morphology scale은 downstream으로 줄어들지 않아야 한다.
5. lake terminal/inlet reach는 canonical Q를 유지하되 lake capacity를 반영한 local shape bounds를
   존중해야 하며 ocean outlet trunk처럼 넓어지면 안 된다.
6. macro field는 broad valley를 주로 읽고, narrow bed를 combined height에 과하게 직접 반영하면 안 된다.
7. heightfield/water는 river bed/water hint를 읽을 수 있지만, hydrology routing을 다시 풀면 안 된다.
8. `river_plan`은 final material이나 voxel fill을 직접 결정하지 않는다.

---

## 현재 구현 상태

- `src/world/generation/river_plan/mod.rs`가 `RiverPlanConfig`, `RiverPlan`, `RiverChain`,
  `RiverReach`, `RiverSegmentPlan`, `RiverPlanStats`, `build_river_plan`을 제공한다.
- 구현은 hydrology selected segment를 모두 보존하면서 selected adjacency를 deterministic chain으로 걷는다.
  source/lake outlet/confluence outflow/topology break에서 시작하고 terminal/lake inlet/sink/outlet/confluence에서
  끝난다.
- hydrology가 selected river의 connected ocean/coast terminal reachability와 selected/display
  discharge monotonicity, lake transition Q propagation을 제공하므로, river_plan은 selected segment를 다시 제거하거나 river path를 새로
  고르지 않는다.
- adjacent segment가 같은 reach type이면 하나의 reach로 묶인다. reach type은 canonical display flow, raw flow,
  terminal role, chain downstream position을 함께 읽으며, hydrology role label만으로 낮은 Q segment를
  `Trunk` reach로 만들지 않는다.
- `upstream_area`, `tributary_flow`, `discharge_q`, `morphology_discharge_q` field는 chain-local ledger와
  diagnostics를 제공한다. ordinary chain에서는 selected/display discharge와 morphology scale이 downstream으로
  줄어들지 않고, lake outlet chain은 inherited canonical Q에서 시작한다.
- bed/broad valley/floodplain/roughness/gravel/cutbank 값은 reach type, chain-local canonical display discharge,
  absolute downstream water-width target, deterministic chain/reach coefficient에서 계산하는 보수적인 guide다. river mouth fan geometry와 final
  water surface solve는 현재 구현하지 않는다.
- low-flow broad valley multiplier는 water/bed width 근처에 머무는 좁은 land-carve shoulder를 우선한다.
  즉 상류 land carve 문제는 주로 shallower carve가 아니라 narrower carve로 해결한다.
- `RiverPlan.segment_endpoints`는 hydrology node position에서 온 `from_position`, `to_position`,
  `downstream_position`을 제공한다.
- `macro_field`는 `RiverPlan`을 selected river edge source와 flow hint source로 소비하지만, plan의
  Q ledger나 U/V 단면 정책을 재구성하지 않는다.
