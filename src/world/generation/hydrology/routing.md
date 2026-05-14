# hydrology routing

## 역할

`routing`은 hydrology stage 안에서 graph corner 기반 물길 원장을 만든다.

이 문서는 아직 코드가 `mod.rs`에서 물리적으로 분리되기 전의 책임 스캐폴딩이다. 구현을 나눌 때
`routing.rs`가 가져가야 할 계약을 작게 정의한다. 상세한 전체 stage 순서와 public output 계약은
`hydrology.md`가 계속 소유한다.

---

## 책임

- macro_map의 corner/site elevation, coast guide, lake candidate context를 읽어 corner elevation을 계산한다.
- 각 corner의 downhill target과 downstream edge를 결정한다.
- graph-stage local minimum을 찾고 `Lake`, `Sink`, `OutletCarve`, `OceanOutlet` resolution으로 분류한다.
- outlet carve가 필요한 경우 downstream edge chain을 routing graph에 반영한다.
- watershed id와 raw flow accumulation을 계산한다.

---

## 비책임

- selected river edge를 고르지 않는다.
- selected graph pruning이나 lake contact marker를 만들지 않는다.
- preview/display/morphology Q를 계산하지 않는다.
- river width, depth, valley shape를 결정하지 않는다.

---

## 입력과 출력

입력:

- `VoronoiGraphPatch`
- `GraphMacroMap`
- `HydrologyConfig`

출력:

- corner elevation
- `downstream: Vec<Option<usize>>`
- `downstream_edges: Vec<Option<VoronoiEdgeId>>`
- local minimum resolution
- watershed id
- raw flow accumulation
- terminal/ocean-coast flags

---

## 불변식

1. raw downstream graph는 deterministic해야 한다.
2. terminal outlet은 connected ocean/coast 의미를 가져야 하며 signed elevation만으로 ocean이 되면 안 된다.
3. local minimum은 반드시 lake, sink, outlet carve, ocean outlet 중 하나로 설명되어야 한다.
4. raw flow accumulation은 display/morphology cap에 오염되지 않는 원장이어야 한다.
5. routing 단계의 raw flow는 selected river visibility와 별개다.

---

## 현재 구현 위치

현재 구현은 `src/world/generation/hydrology/mod.rs` 안에 있다.

- corner elevation과 terminal 판정
- `resolve_downstream`
- `apply_lake_outlets`
- `resolve_watersheds`
- `resolve_flow_accumulation`
- `resolve_terminal_indices`

