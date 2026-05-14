# hydrology discharge

## 역할

`discharge`는 hydrology의 여러 Q 의미를 분리하고, downstream stage가 읽을 canonical river-system Q를
계산한다.

이 문서는 코드 분리 전의 책임 스캐폴딩이다. 최근 hydrology/river_plan 수정의 핵심은 raw Q, selected
river existence, river-system Q, lake-local morphology cap을 같은 값처럼 쓰지 않는 것이다.

---

## Q 용어

- raw Q: routing graph의 catchment/raw flow accumulation 원장이다.
- selected river existence: 어떤 edge가 river로 보일지에 대한 boolean topology 결정이다.
- river-system Q: selected river가 downstream morphology와 preview width에 제공하는 canonical display
  discharge다.
- lake-local cap: lake inlet/outlet 주변 shape를 보수적으로 제한하는 local morphology constraint다.

---

## 책임

- selected graph 위에서 river-system Q를 downstream으로 단조 증가하도록 전파한다.
- explicit tributary가 selected main river에 합류하면 downstream selected segment의 canonical Q가 raw
  accumulation ledger를 통해 그 tributary contribution을 포함하도록 유지한다.
- lake inlet/outlet transition이 river-system Q를 끊거나 작게 reset하지 않도록 한다.
- raw Q를 diagnostic/source ledger로 보존한다.
- lake-local cap을 canonical Q가 아니라 local shape constraint로 유지한다.
- `GraphRiverSegment.flow_accumulation`이 downstream 소비자에게 제공하는 canonical river-system Q가 되도록 보장한다.

---

## 비책임

- raw downhill graph를 만들지 않는다.
- selected river edge를 선택하거나 제거하지 않는다.
- lake edge 금지나 topology pruning을 수행하지 않는다.
- river_plan의 final reach width/depth 값을 직접 계산하지 않는다.

---

## 입력과 출력

입력:

- raw flow accumulation
- pruned selected graph
- downstream graph
- node kind, lake inlet/outlet topology
- lake policy/cap information

출력:

- selected river-system display Q
- raw Q 보존 값

---

## 불변식

1. raw Q는 lake display cap이나 morphology cap 때문에 작아지면 안 된다.
2. river-system Q는 selected downstream path에서 감소하면 안 된다.
3. explicit tributary merge 이후 downstream selected Q는 tributary source의 raw contribution을 잃으면 안 된다.
4. lake inlet/outlet은 valid river-system transition이며 Q ledger reset 지점이 아니다.
5. lake-local cap은 inlet/outlet shape를 제한할 수 있지만 canonical river-system Q를 덮어쓰면 안 된다.
6. river_plan은 `GraphRiverSegment.flow_accumulation`을 canonical Q로 소비한다.

---

## 현재 구현 위치

현재 구현은 `src/world/generation/hydrology/mod.rs` 안에 있다.

- `resolve_selected_flow_accumulation`
- `enforce_monotone_selected_display_flow`
- lake transition Q propagation helpers
- `build_segments`의 `raw_flow_accumulation` / `flow_accumulation` materialization
