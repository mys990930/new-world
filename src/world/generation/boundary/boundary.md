# boundary

## 역할

`boundary`는 polygon boundary, coast, river, biome transition, cliff/fault line의 noisy realization
계약을 소유한다.

raw Voronoi edge는 후보선일 뿐이다. 모든 edge를 visible boundary로 바꾸지 않는다. visible
boundary는 coast, selected river, biome transition, cliff/fault처럼 feature role을 받은 edge만
spline, recursive subdivision, domain warp, feature-specific width와 mask를 통해 현실화한다.

---

## 책임

- Voronoi edge guard geometry 정의
- noisy line / noisy curve deterministic 생성 계약
- coast, river, biome transition, cliff/fault boundary별 realization 정책
- edge id와 seed 기반 cache key 정의
- boundary가 이웃 chunk/region에서 동일하게 재현되도록 입력 계약 유지
- raw graph topology와 visible boundary realization layer 분리

---

## 비책임

- graph topology 생성
- hydrology routing
- macro elevation ownership 결정
- final material policy 선택
- block fill

---

## Noisy Boundary Realization

polygon boundary, coast, river, biome transition은 raw straight line으로 보이면 안 된다.

단순히 noise를 더한 spline은 교차와 찢김을 만들 수 있다. Amit의 noisy edge 아이디어에서 가장
쓸만한 부분은 boundary가 움직일 수 있는 공간을 제한하는 것이다.

하나의 Voronoi edge는 두 site와 두 corner를 가진다. 이 네 점은 boundary guard quadrilateral을
만든다.

- corner edge는 polygon boundary, coast, river 후보선이 된다.
- site edge는 polygon center 사이의 연결선이며 region adjacency와 terrain analysis에 쓸 수 있다.
- noisy line은 guard quadrilateral 안에서 recursive subdivision 또는 spline perturbation으로 만든다.
- 같은 edge id와 seed는 언제나 같은 noisy line을 만든다.
- neighboring chunk가 같은 edge를 샘플하면 같은 line을 얻어야 한다.

---

## 구현 계약

pipeline 7단계는 noisy boundary realization이다. 이 단계는 graph, macro_map, hydrology 결과를 읽어
visible feature edge만 block-space curve로 바꾸고, raw graph topology는 그대로 보존한다.

현재 구현은 `src/world/generation/boundary/mod.rs`에 있다. public entrypoint는 아래다.

```rust
BoundaryConfig::new(seed, generator_version) -> BoundaryConfig
generate_noisy_boundaries(
    &VoronoiGraphPatch,
    &GraphMacroMap,
    &GraphHydrologyGraph,
    BoundaryConfig,
) -> BoundaryCache
```

### 입력 데이터

- `VoronoiGraphPatch`: site, corner, edge id, corner-to-corner straight edge, site-to-site guard geometry
- `GraphMacroMap`: surface kind, coast/ridge/fault guide, signed macro elevation, `MacroLakeEdgeClass`
- `GraphHydrologyGraph`: selected river segment, selected/display discharge, inlet/outlet/sink/coast outlet node
- stage config: boundary seed salt, feature별 amplitude, subdivision depth, guard margin, smoothing policy

### 출력 데이터 계약

출력은 id 기반 annotation layer다.

```rust
BoundaryCache {
    curves: Vec<NoisyBoundaryCurve>,
    stats: BoundaryStats,
}

NoisyBoundaryCurve {
    edge: VoronoiEdgeId,
    role: BoundaryRole,
    anchors: BoundaryAnchors,
    points: Vec<WorldPlanePoint>,
    width_hint_blocks: f32,
    amplitude: f32,
    seed: u64,
    guard: BoundaryGuard,
}

BoundaryRole::{
    Coast,
    River,
    Ridge,
    Fault,
    LakeShore,
    LandBoundary,
}
```

`points`는 world-space polyline/spline control point다. 이후 field/heightfield 단계는 이 curve와
edge id mapping을 읽어 coast gradient, river corridor, ridge envelope, lake shore mask를 만든다.
`BoundaryStats`는 role별 curve 수, guard violation 수, lake edge 위 river curve 수, river endpoint
attachment mismatch 수를 제공한다.

### Deterministic Seed Policy

- curve seed는 `(world seed, generator version, edge id, role salt)`로 만든다.
- 같은 edge id와 role은 patch 요청 중심, chunk 요청 순서, worker thread scheduling에 관계없이 같은
  point sequence를 만든다.
- 병렬 생성은 허용하지만 최종 `curves`는 `(role, edge id)` 기준으로 정렬한다.
- 같은 edge라도 `Coast`, `River`, `LakeShore`처럼 role이 다르면 salt가 달라 다른 noisy path를 가진다.

### 알고리즘

구현은 Amit의 noisy edge 아이디어를 launch 수준으로 보수적으로 적용한다.

- 하나의 Voronoi edge는 두 corner와 두 site center를 함께 읽는다.
- 이 네 점의 bounding guard를 만들고, margin을 더해 수치적 guard 영역을 만든다.
- corner-to-corner edge를 recursive midpoint displacement polyline으로 세분화한다.
- noisy point는 edge normal 방향으로 흔들되, guard 영역으로 clamp한다.
- endpoint는 항상 원본 corner 위치를 유지한다.
- 기본 subdivision level은 4이며 curve당 17개의 point를 만든다.
- amplitude는 role별로 다르며 0.5 이하로 제한한다.

기본 amplitude:

- coast: `0.24`
- river: `0.16`
- lake shore: `0.18`
- ridge: `0.10`
- fault: `0.08`
- land boundary: `0.07`

이 방식은 Amit의 사각형 내부 recursive subdivision을 그대로 베낀 것은 아니지만, 같은 핵심 원칙을
따른다. 즉 noisy line은 raw Voronoi topology를 바꾸지 않고, 각 edge의 두 corner와 두 site가 만드는
guard 안에서만 흔들린다. 이후 더 정교한 point-in-convex-quad clamp가 필요해지면 `BoundaryGuard`를
axis-aligned guard에서 bilinear/convex guard로 좁힐 수 있다.

### Feature Constraints

- river curve는 hydrology selected segment에 대해서만 생성한다. macro river potential이나 raw graph edge는
  river가 아니다.
- selected river는 계속 `MacroLakeEdgeClass::NonLake` edge만 사용한다. lake boundary/internal/adjacent
  edge에는 river curve를 만들지 않는다.
- `LakeInlet`/`LakeOutlet`은 land-side selected endpoint와 lake component를 연결하는 접합 anchor다.
  river curve는 endpoint에서 끝나거나 시작하고, lake boundary curve는 별도 lake shore role로 생성한다.
- coast는 connected ocean basin과 land ownership 경계에서만 생성한다. lake shore와 ocean coast는 role을
  분리해 amplitude와 material mask를 다르게 준다.
- coast amplitude는 river보다 크고, ridge/fault는 feature 방향성을 유지하도록 낮은 lateral noise와
  sharpness hint를 가진다.
- noisy point는 edge guard quadrilateral 안에 있어야 하며, 이웃 edge curve와 교차하면 안 된다.

### Runtime Cache

runtime cache chain은 아래 순서를 따른다.

```text
graph region cache
-> macro map cache
-> hydrology cache
-> boundary cache
-> heightfield cache
-> chunk generation samples column/window data
```

chunk fill은 boundary curve를 새로 만들지 않고 boundary cache를 샘플한다. cache miss는 worker에서
graph/macro/hydrology와 같은 deterministic key/padding 정책으로 생성한다.

### Preview

- `macro_map_preview`는 같은 world window에서 faint raw Voronoi edge와 noisy boundary를 함께 보여준다.
- 현재 layer는 before/after overlay를 제공한다.
  - faint raw Voronoi edge
  - straight coast/ridge/fault guide
  - coast noisy curve
  - river noisy curve, width hint
  - lake shore curve
  - ridge/fault curve
- PNG metadata에는 curve count, role별 count, guard violation count, lake edge river curve count,
  hydrology endpoint attachment mismatch count를 기록한다.

현재 preview는 selected river straight segment를 기본 표시에서 제외하고, boundary의 noisy river curve를
river overlay로 사용한다. hydrology node와 inlet/outlet arrow는 그대로 마지막 layer에 그린다.

### 테스트

- determinism: 같은 seed/config/edge role은 같은 curve point를 만든다.
- adjacent patch stability: 인접 graph patch overlap의 같은 edge curve가 동일해야 한다.
- guard containment: noisy points는 edge guard quadrilateral과 margin 안에 있어야 한다.
- no crossing: 같은 role 또는 서로 다른 visible role curve가 guard 밖 교차를 만들지 않아야 한다.
- hydrology attachment: river curve endpoint는 selected segment endpoint, `LakeInlet`, `LakeOutlet`,
  `CoastOutlet`, `Sink` node와 계속 붙어 있어야 한다.
- lake constraint: lake edge에는 river curve가 생성되지 않는다.

---

## Feature별 표현

boundary 표현은 feature마다 다를 수 있다.

- biome boundary: gradient, dithering, domain warp 중심
- river: spline corridor, width, floodplain, gravel bar, wetland mask
- coast: noisy coastline, beach/cliff/rocky shore material policy
- fault/cliff: 부분적인 discontinuous height transition 허용
- lake edge: water level flattening과 shore material transition

---

## 불변식

1. raw Voronoi edge가 그대로 직선 river/coast/biome boundary로 보이면 안 된다.
2. feature role이 없는 edge는 visible noisy boundary를 만들지 않는다.
3. noisy boundary는 raw graph topology를 대체하지 않는다.
4. noisy boundary는 edge guard 영역 밖으로 나가거나 이웃 edge와 교차하면 안 된다.
5. 같은 edge id, seed, feature role은 같은 curve를 만들어야 한다.
6. boundary realization은 chunk 요청 순서와 graph patch padding 차이에 독립적이어야 한다.
7. hard owner와 visible material boundary가 과도하게 같은 선을 따라가면 회귀다.
