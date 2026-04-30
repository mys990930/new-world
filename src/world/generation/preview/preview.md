# preview

## 역할

`preview`는 generation stage별 topdown preview binary의 입력/출력 계약을 소유한다.

초기 generator에서는 기능 완성도만큼 진단 가능성이 중요하다. 각 stage는 chunk 생성 없이도 같은
입력으로 같은 preview를 만들 수 있어야 한다.

---

## 책임

- stage별 preview input area, seed, generator version, config 계약
- deterministic PNG 또는 raw dump 출력 계약
- graph, macro map, hydrology, boundary, field, heightfield, surface 결과를 독립적으로 검사할 수 있는 surface 정의
- artifact regression을 테스트와 연결할 수 있는 기준 제공

---

## 비책임

- GPU rendering
- interactive editor UI
- runtime chunk storage mutation
- generation stage 의미 소유

---

## Preview 대상

초기 preview 우선순위:

1. site/corner/edge graph preview
2. graph region ownership preview
3. continent/ocean ownership preview
4. Voronoi macro elevation preview
5. mountain/ridge/fault/coast edge preview
6. dominant site map
7. blended influence map
8. elevation/corner downhill arrow map
9. watershed map
10. river flow accumulation map
11. noisy edge preview
12. Voronoi-derived macro noise/gradient map preview
13. Perlin micro relief preview
14. heightfield and water surface preview
15. final temperature/hydration/biome influence preview
16. biome/material/surface plan preview
17. vegetation placement preview
18. voxel fill preview

---

## Determinism

- 같은 seed, generator version, area, stage input은 같은 preview를 만든다.
- preview는 chunk 생성 없이도 stage 결과를 검사할 수 있어야 한다.
- 이미지 출력뿐 아니라 stage별 raw dump가 필요하면 같은 deterministic 입력 계약을 사용한다.
- stage artifact를 발견하면 문서, 테스트, 구현 중 어느 층의 가정이 깨졌는지 추적한다.

---

## Artifact 기준

- graph region 사각 경계가 보이면 회귀다.
- raw Voronoi edge가 그대로 river/coast/biome boundary로 보이면 회귀다.
- noisy boundary가 guard 영역 밖으로 나가면 회귀다.
- hydrology flow가 outlet 없이 끊기면 회귀다.
- biome/material transition이 hard owner 선을 그대로 따라가면 회귀다.

---

## 불변식

1. preview binary는 stage 결과를 chunk 생성 없이 검사할 수 있어야 한다.
2. preview output은 deterministic이어야 한다.
3. preview는 문서와 테스트의 보조물이 아니라 generation artifact를 발견하는 1차 검증 표면이다.
4. 이상한 작은 흔적이 보이면 무시하지 않고 source-of-truth 문서와 테스트로 환류한다.
