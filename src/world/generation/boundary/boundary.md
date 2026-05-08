# boundary

## 역할

`boundary`는 모든 Voronoi edge의 canonical noisy geometry layer를 소유한다.

raw Voronoi edge는 graph topology와 semantic annotation의 기준이지만, 최종 지형에서 그대로
보이는 직선이어서는 안 된다. 이 모듈은 특정 feature edge만 골라 noisy curve를 만드는 단계가
아니다. graph patch에 포함된 모든 Voronoi edge에 대해 deterministic noisy curve를 만들고, 이후
coast, ridge, fault, river, lake, biome/material transition, heightfield mask가 같은 edge id의
curve를 따라 해석되도록 한다.

---

## 책임

- 모든 Voronoi edge에 대한 noisy polyline/spline 생성
- Voronoi edge guard geometry 정의
- edge id와 seed 기반 deterministic cache key 정의
- 인접 chunk/region/patch가 같은 edge를 같은 curve로 재현하도록 입력 계약 유지
- edge별 profile/amplitude/constraint parameter 결정
- raw graph topology와 visible/sample geometry layer 분리

---

## 비책임

- graph topology 생성
- hydrology routing 또는 selected river chain 결정
- macro elevation ownership 결정
- final material policy 선택
- block fill
- river 전용 noisy curve 생성

---

## Canonical Noisy Edge Layer

pipeline 8단계는 noisy boundary realization이다. 이 단계의 출력은 graph edge 전체에 대한
canonical geometry annotation이다.

```rust
BoundaryConfig::new(seed, generator_version) -> BoundaryConfig
generate_noisy_boundaries(
    &VoronoiGraphPatch,
    &GraphMacroMap,
    BoundaryConfig,
) -> BoundaryCache
```

출력 계약:

```rust
BoundaryCache {
    curves: Vec<NoisyBoundaryCurve>,
    stats: BoundaryStats,
}

NoisyBoundaryCurve {
    edge: VoronoiEdgeId,
    profile: BoundaryProfile,
    anchors: BoundaryAnchors,
    points: Vec<WorldPlanePoint>,
    amplitude: f32,
    seed: u64,
    guard: BoundaryGuard,
}
```

불변식은 `macro_map.edges.len() == boundary.curves.len()`이다. downstream stage는 coast, ridge,
fault, lake shore를 새 curve로 다시 만들지 않고, 각 feature가 참조하는 `VoronoiEdgeId`의
`NoisyBoundaryCurve`를 읽는다. river도 boundary 전용 noisy curve를 만들지는 않지만, macro_field의
visible carve corridor는 hydrology chain/corner anchor에서 파생한 spline/corridor일 수 있다. 이때
canonical noisy edge는 selected topology와 corridor reference로 남는다.

---

## Edge Profile

모든 edge에는 noisy curve가 있다. 역할별 차이는 curve의 존재 여부가 아니라 profile/amplitude/
constraint parameter에 반영한다.

- `Ordinary`: 일반 graph edge, biome/material blending이나 field sampling의 낮은 amplitude 기준
- `Coast`: connected ocean과 land 사이의 shoreline profile
- `Lake`: lake boundary/internal/lake-adjacent edge profile
- `Ridge`: ridge guide가 붙은 edge, heightfield ridge envelope가 읽을 더 sharp한 profile
- `Fault`: fault guide가 붙은 edge, lateral noise는 낮고 discontinuity hint를 유지하는 profile
- `LandSeam`: land-owned surface kind가 바뀌는 낮은 amplitude seam

profile 우선순위는 launch 기준으로 `Coast > Lake > Ridge > Fault > LandSeam > Ordinary`다. 한 edge에
여러 semantic guide가 붙을 수 있어도 canonical curve는 하나이며, 필요하면 이후 stage가 같은 curve를
여러 mask로 해석한다.

---

## River Contract

river는 별도 `BoundaryRole::River` curve를 만들지 않는다.

hydrology의 `GraphRiverSegment`는 selected edge id path다. boundary 단계는 이 path를 위한 별도
river curve를 만들지 않는다. preview overlay와 topology/corridor reference는 river segment의 `edge`
id로 `BoundaryCache.curve_for_edge(edge)`를 찾는다.

단, macro_field의 valley carve shape는 canonical noisy edge polyline을 raw 그대로 centerline으로
굽는 것으로 고정하지 않는다. bend에서 thick capsule union이 둥근 blob처럼 부푸는 문제를 줄이고
corner anchor만 잇는 직선 조각도 피하기 위해, macro_field는 selected segment topology를 chain
단위로 읽고 각 selected edge의 `NoisyBoundaryCurve.points`를 downstream 방향으로 이어 붙인 뒤
resample/smoothing한 derived river spline/corridor를 만들 수 있다. 이 spline은 selected graph
path에서 과하게 벗어나면 안 되며, boundary cache는 여전히 raw topology와 visible boundary reference를
소유한다.

lake rule은 그대로 유지한다.

- lake boundary/internal/lake-adjacent edge에도 noisy curve는 존재한다.
- selected river segment는 `MacroLakeEdgeClass::NonLake` edge만 사용할 수 있다.
- inlet/outlet은 noisy edge endpoint 또는 land-side endpoint anchor에 접합한다.
- lake edge 위로 river를 그리거나 sampling해서는 안 된다.

---

## Deterministic Seed Policy

- curve seed는 `(world seed, generator version, edge id, profile salt)`로 만든다.
- 같은 edge id와 profile은 patch 요청 중심, chunk 요청 순서, worker scheduling에 관계없이 같은 point
  sequence를 만든다.
- 병렬 생성은 허용하지만 최종 `curves`는 `edge id` 기준으로 정렬한다.
- profile은 macro/hydrology 결과를 읽는 downstream 의미이며, canonical curve identity는 edge id가
  소유한다.

---

## 알고리즘

launch 구현은 Amit식 noisy edge의 핵심인 "edge가 움직일 수 있는 guard를 제한한다"는 원칙을 따른다.

- 하나의 Voronoi edge는 두 corner와 두 site center를 함께 읽는다.
- 이 네 점의 guard를 만들고 margin을 더해 수치적 guard 영역을 만든다.
- corner-to-corner edge를 충분히 촘촘한 polyline으로 세분화한 뒤, 이 polyline을 correlated
  displacement curve로 해석한다.
- noisy point는 직선 edge 위에 sample만 찍는 것이 아니라, edge tangent의 법선 방향으로 실제
  world-space displacement를 적용한다. 즉 noisy boundary의 정의는 "직선 segment를 더 촘촘히 그린
  것"이 아니라, endpoint anchor는 유지하면서 중간 control/sample point가 좌우로 울퉁불퉁하게 흔들린
  polyline이다.
- displacement는 low/mid frequency coherent wave, 몇 개의 deterministic value-noise knot,
  약한 high-frequency wave를 합성한 뒤 smoothing pass를 거친다. sample마다 독립 jitter를 강하게
  넣지 않는다. 독립적인 salt-and-pepper offset은 자연스러운 coastline/field boundary가 아니라
  톱니 모양 polyline으로 보이기 쉽기 때문이다.
- endpoint에서는 displacement가 0으로 줄어드는 smooth falloff를 적용한다. endpoint anchor는 graph
  topology와 adjacent patch stability를 위해 고정하고, interior만 더 크게 요동할 수 있다.
- noisy point는 edge normal 방향으로 흔들되 guard 영역으로 clamp한다. 한쪽 normal 방향이 guard에
  눌려 직선으로 붕괴하면 반대 방향 후보를 사용해 유효한 perpendicular displacement를 유지한다.
- endpoint는 항상 원본 corner 위치를 유지한다.
- 기본 subdivision level은 6이며 curve당 65개의 point를 만든다. 점 수는 raw topology를 바꾸는
  것이 아니라 preview/heightfield가 더 부드러운 곡선을 샘플할 수 있게 하는 geometry layer다.
- amplitude profile 값은 edge/site local scale에 곱해지는 비율이며, launch 기본값은 4K preview에서도
  식별 가능한 최소 world-space displacement를 보장하기 위해 30 block의 최소 visible amplitude floor를
  가진다. 단, 아주 짧은 degenerate edge는 edge length/site span 기반 clamp가 우선한다.
- amplitude는 edge length와 site span 대비 과도하게 커지지 않도록 clamp하고, launch 기본값에서는
  160 block의 절대 상한을 둔다.

기본 amplitude:

- ordinary: `0.18`
- coast: `0.39`
- lake: `0.32`
- ridge: `0.30`
- fault: `0.21`
- land seam: `0.21`

기본 preview scale에서 기대값:

- 4K 기본 preview(`32768` block span, `3840` px width)는 1px이 약 8.53 block이다.
- 기본 curve 평균 amplitude는 여러 profile을 합쳐 대략 수십 block 단위여야 하며, 평균 visible
  perpendicular displacement가 4K에서 약 2px 이상으로 드러나야 한다.
- `BoundaryStats`는 평균/최대 amplitude block, 평균/최대 perpendicular displacement block,
  nearly-straight curve count를 기록한다. nearly-straight는 의미 있는 길이의 edge가 guard/clamp나
  잘못된 noise 합성 때문에 거의 직선으로 남은 경우를 찾는 회귀 계측이다.

이 구현은 raw topology를 바꾸지 않는다. graph edge id, corner id, site id는 그대로 유지되고, noisy
curve는 heightfield와 preview가 읽는 geometry layer일 뿐이다.

---

## Runtime Cache

runtime cache chain은 아래 순서를 따른다.

```text
graph region cache
-> macro map cache
-> hydrology cache
-> final cell context cache
-> boundary cache
-> macro field / heightfield cache
-> chunk generation samples column/window data
```

`BoundaryCache`는 `graph edge id -> noisy polyline/spline` 전체를 저장한다. chunk fill은 boundary
curve를 새로 만들지 않고 boundary cache를 샘플한다. cache miss는 worker에서 graph/macro/hydrology/final-cell-context와
같은 deterministic key/padding 정책으로 생성한다.

---

## Preview

`macro_map_preview`는 raw Voronoi edge와 canonical noisy edge를 함께 보여줄 수 있어야 한다.

- faint raw Voronoi edge: topology 진단용 straight edge
- canonical noisy edge: 모든 edge에 존재하는 noisy geometry
- coast/ridge/fault/lake overlay: 별도 curve가 아니라 해당 edge id의 canonical curve를 따라 그림
- river overlay: topology reference는 selected edge id의 canonical curve를 사용하되, macro_field
  river valley/carve channel은 chain-level derived spline/corridor로 보일 수 있다.
- river topology/flow overlay: hydrology selected/display `flow_accumulation`을 사용하되 topology
  reference는 `BoundaryCache.curve_for_edge(segment.edge)`를 사용한다. macro_field의 carve channel은
  이 reference를 바탕으로 별도 derived spline/corridor를 표시할 수 있다.

PNG metadata에는 total curve count, profile별 count, guard violation count, missing macro edge count,
평균/최대 amplitude block, 현재 preview scale에서의 평균/최대 pixel displacement, nearly-straight
curve count를 기록한다. river curve count나 river endpoint mismatch 같은 항목은 river 전용 boundary
curve가 없으므로 boundary stats의 책임이 아니다. river/lake 접촉 정합성은 hydrology stats가 계속
소유한다.

---

## 테스트

- all-edge coverage: macro edge마다 정확히 하나의 noisy curve가 있어야 한다.
- determinism: 같은 seed/config/edge/profile은 같은 curve point를 만든다.
- adjacent patch stability: 인접 graph patch overlap의 같은 edge curve가 동일해야 한다.
- guard containment: noisy points는 edge guard와 margin 안에 있어야 한다.
- visible displacement: non-degenerate edge는 interior point가 원본 straight segment와 같은 직선 위에
  머물면 안 되며, 기본 amplitude는 4K preview scale에서 식별 가능해야 한다.
- smoothness: adjacent sample의 normal displacement가 독립 jitter처럼 급격히 튀지 않아야 한다.
  구현은 평균 second-difference를 테스트해 톱니형 polyline 회귀를 잡는다.
- no duplicate river curve: hydrology selected segment가 boundary curve 수를 늘리면 안 된다.
- lake constraint: lake edge에도 canonical noisy curve는 있지만 selected river segment는 해당 edge를
  사용할 수 없다.

---

## 불변식

1. 모든 macro Voronoi edge는 canonical noisy curve를 하나 가진다.
2. raw Voronoi edge가 그대로 직선 river/coast/biome boundary로 보이면 안 된다.
3. river는 별도 noisy curve를 만들지 않는다. selected edge id path는 canonical noisy geometry를
   topology/corridor reference로 사용하고, macro_field carve shape는 별도 derived spline/corridor로
   해석될 수 있다.
4. noisy boundary는 raw graph topology를 대체하지 않는다.
5. noisy point는 edge guard 영역 밖으로 나가거나 이웃 edge와 교차하면 안 된다.
6. 같은 edge id, seed, profile은 같은 curve를 만들어야 한다.
7. boundary realization은 chunk 요청 순서와 graph patch padding 차이에 독립적이어야 한다.
8. hard owner와 visible material boundary가 과도하게 같은 직선을 따라가면 회귀다.
