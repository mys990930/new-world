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
