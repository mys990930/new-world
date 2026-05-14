# hydrology selection

## 역할

`selection`은 raw routing graph 위에서 어떤 downstream edge가 preview-visible selected river가 될지
고른다.

이 문서는 코드 분리 전의 책임 스캐폴딩이다. 전체 hydrology public contract는 `hydrology.md`가
소유하고, 이 문서는 selected river 선택 정책을 빠르게 찾기 위한 진입점이다.

---

## 책임

- terrain-like headwater source 후보를 찾는다.
- source 후보의 downstream path potential, source hydration, highland/ridge/local-maximum context를 평가한다.
- 일반 river threshold와 lake inlet/terminal threshold를 적용한다.
- mainstem 선택 뒤 별도 tributary source threshold/density 정책으로 side source 후보를 평가한다.
- tributary source 후보가 already-selected main river에 bounded downhill path로 합류하는지 확인한다.
- tributary끼리 먼저 만나거나 같은 unselected path를 공유하는 후보는 deterministic score 순서로 하나만 남긴다.
- explicit tributary 후보가 hydration coherence 때문에 매우 가까운 source/초기 path/merge neighborhood로
  몰리면, score 순서로 가장 좋은 후보를 먼저 수락하고 이후 후보를 world-block spacing 기준으로 억제한다.
- 선택된 source에서 downstream path를 따라 selected segment 후보를 만든다.
- lake edge, lake boundary edge, lake-adjacent edge를 selected river로 선택하지 않는다.

---

## 비책임

- raw downhill graph를 만들지 않는다.
- selected graph의 multi-incoming 충돌을 최종 정리하지 않는다.
- canonical river-system Q를 계산하지 않는다.
- river_plan reach morphology를 정하지 않는다.

---

## 입력과 출력

입력:

- routing output: downstream, downstream edge, raw flow, terminal, local minimum resolution
- lake policy input: lake candidate, lake inlet policy, lake contact topology
- graph adjacency and macro edge lake class

출력:

- `selected: Vec<bool>` candidate set
- selected lake-chain budget usage

---

## 불변식

1. selected river start는 단순 threshold crossing이나 임의 border가 아니라 terrain source 후보여야 한다.
2. `river_flow_threshold`는 mainstem downstream discharge potential을 고르는 기준이며 tributary 개수 조절
   수단이 아니다.
3. `tributary_source_threshold` / `tributary_source_hydration`을 낮추면 mainstem 길이를 늘리지 않고
   selected main river에 붙는 tributary source 수가 늘어날 수 있다.
4. explicit tributary는 downstream path를 따라 이미 선택된 main river에 닿아야 하며, 다른 tributary와
   먼저 교차하거나 path를 공유하면 안 된다.
5. explicit tributary source는 `tributary_source_min_spacing_blocks`보다 가까운 이미 수락된 tributary
   source와 같이 남으면 안 된다. 초기 downstream path가 여러 edge 동안
   `tributary_parallel_path_min_spacing_blocks` 안에서 나란히 흐르거나 같은 local merge target
   neighborhood에 붙는 경우도 후순위 후보를 억제한다.
6. 선택된 river는 downstream path를 따라 terminal, lake policy endpoint, 또는 mainstem merge point까지
   이어져야 한다.
7. selection은 raw flow 원장을 수정하지 않는다.
8. lake 관련 edge는 selected river edge가 될 수 없다.
9. 너무 작은 lake feeder는 marker/selected river로 승격되지 않을 수 있지만, raw flow 원장에는 남아야 한다.

---

## 현재 구현 위치

현재 구현은 `src/world/generation/hydrology/mod.rs` 안에 있다.

- `select_river_paths`
- `headwater_source_candidate`
- explicit tributary candidate/path helpers
- source scoring helpers
- accepted tributary source/path spacing suppression helpers
- lake terminal/inlet selection threshold helpers
