# surface_plan

## 역할

`surface_plan`은 heightfield와 continuous field, hydrology role을 읽어 biome/material/water/coast
surface policy를 resolve하는 계약을 소유한다.

이 단계는 visible material boundary를 안정적으로 만들지만, hard polygon owner를 그대로 색칠하지
않는다.

---

## 책임

- biome influence와 hydrology role에서 surface policy 선택
- ocean, coast, lake, river, wetland, floodplain의 material 의미 구분
- slope, exposure, elevation, hydration에 따른 beach/cliff/rock/soil/vegetation 전환
- column voxel fill이 읽을 surface column plan 정의
- seasonal/runtime surface state와 연결될 수 있는 world-side 계약 유지

---

## 비책임

- graph construction
- hydrology solve
- final height 계산
- block storage mutation
- renderer material upload

---

## Resolve 입력

surface resolve는 아래 입력을 함께 본다.

- continuous temperature / hydration / ruggedness
- dominant site와 blended biome influence
- heightfield surface y와 slope
- ocean/lake/river/wetland/coast role
- river flow accumulation과 floodplain width
- ridge/fault/cliff guide
- local soil/sediment class
- runtime season/weather state가 허용하는 override

---

## Material Transition

visible material boundary는 hard owner 경계를 그대로 따라가면 안 된다.

권장 방식:

- continuous field gradient
- domain-warped boundary distance
- deterministic dithering
- cover override
- hydrology role 우선순위
- slope/exposure 기반 rocky override

---

## 불변식

1. biome owner와 visible material boundary는 분리될 수 있어야 한다.
2. ocean, lake, river, wetland, coast는 같은 water mask로 뭉개지면 안 된다.
3. material transition은 chunk 경계와 graph region 경계에 독립적이어야 한다.
4. surface plan은 renderer 리소스를 소유하지 않는다.
5. surface plan은 `ChunkData`를 직접 수정하지 않고 voxel fill이 소비할 계획을 만든다.
