# hydrology topology

## 역할

`topology`는 selected river 후보 graph를 public hydrology result로 내보내기 전에 정리하고 검증한다.

이 문서는 코드 분리 전의 책임 스캐폴딩이다. hydrology 전체 stage order와 public type 계약은
`hydrology.md`가 계속 소유한다.

---

## 책임

- selected river가 lake internal/boundary/adjacent edge를 쓰지 않도록 강제한다.
- `LakeInlet`과 `LakeOutlet` marker가 land-side endpoint에만 생기도록 정리한다.
- selected graph의 multi-incoming/shared-corner 충돌을 정리하되, 두 incoming이 하나의 outgoing으로
  합류하는 정상 confluence는 보존한다.
- coast-reaching selected river는 land/coast terminal endpoint에서 종료한다.
  topology는 non-lake ocean-owned edge를 추가 selected segment로 연장하지 않으며, downstream stage가
  하구 표현을 만들더라도 selected hydrology topology 밖의 heightfield-local 처리로 다룬다.
- 서로 다른 corner index가 같은 Voronoi corner id로 materialize되는 경우에도 public selected graph는
  그 corner id에서 하나의 downstream selected continuation만 갖도록 정리한다.
- graph corner id가 달라도 같은 world-space 위치에 겹쳐 보이는 selected endpoint group은 하나의
  preview-visible confluence로 정리한다.
- repeated lake contact chain을 제거한다.
- selected fragment가 valid terminal이나 documented lake endpoint를 잃으면 제거한다.
- topology validation stats를 계산한다.

---

## 비책임

- raw downstream graph를 만들지 않는다.
- source 후보를 선택하지 않는다.
- canonical river-system Q를 계산하지 않는다.
- river width/depth/morphology를 결정하지 않는다.

---

## 입력과 출력

입력:

- selected candidate set
- routing downstream graph
- lake contact topology
- macro edge lake class
- local minimum resolution

출력:

- pruned selected set
- `GraphDrainageNodeKind` classification
- `GraphHydrologyTopologyStats`

---

## 불변식

1. preview-visible selected river graph는 unexplained shared-corner intersection을 남기면 안 된다.
2. selected river segment는 lake internal/boundary/adjacent edge를 쓰면 안 된다.
3. lake contact는 selected segment endpoint의 `LakeInlet`/`LakeOutlet` marker로만 표현한다.
4. selected river confluence는 최대 두 incoming segment와 정확히 하나의 outgoing selected segment로만
   표현한다. 세 개 이상의 incoming branch나 tributary끼리 먼저 만나는 shared path는 제거한다.
   같은 Voronoi corner id에서 selected outgoing segment가 둘 이상 materialize되면 strongest downstream
   continuation 하나만 남긴다. 합류 후 visible river가 다시 둘 이상으로 갈라지면 안 된다.
   같은 world-space 위치에 겹친 corner id group도 incoming이 있으면 group 밖으로 나가는 selected
   continuation은 정확히 하나여야 한다.
5. ordinary selected fragment는 valid downstream selected path를 잃으면 제거한다.
6. topology pass는 raw flow accumulation을 보존한다.

---

## 현재 구현 위치

현재 구현은 `src/world/generation/hydrology/topology.rs` 안에 있다.

- `enforce_lake_contact_topology`
- `remove_invalid_terminal_intersections`
- `prune_multi_incoming_selected_branches`
- `prune_duplicate_corner_outgoing_selected_branches`
- `remove_repeated_lake_contact_chains`
- `prune_disconnected_selected_fragments`
- `resolve_node_kinds`
- `resolve_topology_stats`
