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
2. 선택된 river는 downstream path를 따라 terminal 또는 lake policy endpoint까지 이어져야 한다.
3. selection은 raw flow 원장을 수정하지 않는다.
4. lake 관련 edge는 selected river edge가 될 수 없다.
5. 너무 작은 lake feeder는 marker/selected river로 승격되지 않을 수 있지만, raw flow 원장에는 남아야 한다.

---

## 현재 구현 위치

현재 구현은 `src/world/generation/hydrology/mod.rs` 안에 있다.

- `select_river_paths`
- `headwater_source_candidate`
- source scoring helpers
- lake terminal/inlet selection threshold helpers

