# field

## 역할

`field`는 Voronoi graph에서 파생된 continuous sample 계약을 소유한다.

visible terrain은 hard polygon id를 직접 샘플하지 않는다. gameplay나 cache identity를 위해
dominant owner는 유지할 수 있지만, 높이, 바이옴, 재질, 수분, 온도는 주변 graph influence와
장거리 field를 섞은 연속 값으로 해석한다.

---

## 책임

- world-space column 주변의 blended graph influence 표현
- temperature, hydration, elevation, continentality, ruggedness, oceanness, mountainness field 표현
- influence weight normalization helper 제공
- hydration/moisture, biome influence, material transition의 입력 field 계약 정의

---

## 비책임

- Voronoi graph 생성
- hydrology routing
- noisy boundary curve 생성
- final biome id 확정
- block fill

---

## Continuous Region Fields

world-space column sampling은 주변 site/corner의 influence를 섞어 continuous field를 만든다.

주요 field:

- temperature
- hydration
- elevation bias
- continentality
- ruggedness
- oceanness
- mountainness
- basinness
- coastness
- fresh-water proximity

인접 site끼리는 완전 랜덤 값이 아니라 어느 정도 연속성을 가져야 한다. field는 한 번에 끝나는
단계가 아니라 두 층으로 나뉜다.

- base graph field: graph 생성 직후 site/corner에 temperature, humidity, continentality, elevation seed를 부여하고 이웃 smoothing한다.
- final column field: heightfield와 hydrology 이후 elevation, water proximity, rain shadow, river/lake/wetland proximity를 반영해 temperature/hydration/biome influence를 다시 resolve한다.

방법 후보:

- graph neighbor smoothing
- low-frequency noise를 site seed에 더하기
- climate band / latitude / prevailing wind 같은 장거리 field를 site 값에 반영
- watershed, coast, mountain chain 같은 graph-derived field를 후처리로 합성

중요한 점은 `dominant_site`와 `visible field`를 분리하는 것이다. gameplay query는 안정적인
owner를 원할 수 있지만, 화면에 보이는 바이옴/재질/높이는 blended field를 먹어야 한다.

---

## Moisture And Hydration

hydration은 site random value 하나로 결정하지 않는다.

입력 후보:

- base climate humidity
- latitude/temperature
- prevailing wind와 rain shadow
- distance to ocean
- distance to fresh water
- lake/wetland proximity
- river flow accumulation
- elevation
- local soil/sediment class

Amit의 mapgen2는 강과 호수에서 멀어질수록 moisture가 줄어들게 했다. 이 방식은 단순하지만
바이옴 설득력이 좋다. launch 단계에서는 아래 모델을 우선 고려한다.

```text
hydration =
    base_humidity_field
  + fresh_water_proximity
  + wetland_bonus
  + rainfall_bonus
  - rain_shadow
  - aridity_bias
```

moisture는 원하는 분포로 redistribution할 수 있다. 예를 들어 너무 건조하거나 너무 습한 지역이
몰리면 graph patch 단위로 percentile remap을 적용할 수 있다. 단, 무한 월드에서는 patch 경계가
보이지 않도록 region padding과 deterministic window 규칙이 필요하다.

---

## Biome Influence

biome은 최종적으로 continuous field에서 resolve한다.

기본 축:

- temperature
- hydration
- elevation
- hydrology role
- coast/ocean/lake state
- ruggedness
- dominant graph owner

Whittaker diagram류의 temperature/moisture 2D 분류는 좋은 출발점이다. 하지만 이 프로젝트에서는
polygon 하나가 반드시 하나의 biome일 필요가 없다.

권장 방식:

- site는 dominant biome owner를 가진다.
- corner/edge/column은 blended biome influence를 가진다.
- material policy는 owner + local field + hydrology role을 함께 본다.
- 경계는 hard edge가 아니라 gradient, dithering, domain warp, cover override로 표현한다.

예외:

- gameplay상 안정적인 지역 판정이 필요한 경우 hard owner를 제공할 수 있다.
- save/load나 minimap cache가 안정적인 region id를 원할 수 있다.
- 이 경우에도 visible material boundary는 hard owner 경계를 그대로 따라가면 안 된다.

---

## 불변식

1. hard polygon ownership은 cache/query identity로 존재할 수 있지만, visible terrain은 blended field를 샘플해야 한다.
2. influence weight는 downstream terrain synthesis 전에 normalize되어야 한다.
3. graph boundary distance는 diagnostic과 shaping input일 뿐이며, visible output에 쓰기 전에 warp와 blend를 거쳐야 한다.
4. biome owner와 visible material boundary는 분리될 수 있어야 한다.
5. ocean/coast/lake/wetland 구분은 material policy와 topdown preview에서 일관되어야 한다.

---

## 현재 구현 상태

- 현재는 sampling contract scaffold 단계다.
- future work는 graph-neighborhood sampling, spline-warped boundary distance field, cached field patch를 추가해야 한다.
