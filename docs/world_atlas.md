# world_atlas.md

## 1. 문서 목적

이 문서는 청크 생성 이전 단계에서 월드의 거시 구조를 계산하는 `World Atlas` 계층을 정의한다.

World Atlas의 역할은 다음과 같다.

1. 시드 하나로부터 대륙/해양 구조를 결정한다.
2. 거시 고도, 산맥, 수계, 기후 필드를 계산한다.
3. 각 atlas cell의 환경 축과 overlay를 계산한다.
4. biome map, climate map, ridge map 등 디버그 가능한 2D 필드를 제공한다.
5. 이후 `biome_resolver.md`와 `chunk_realization.md`의 입력 데이터를 제공한다.

Atlas는 실제 블록을 채우지 않는다.  
Atlas는 “이 좌표권이 어떤 환경 성격을 가지는가”를 먼저 정하는 계층이다.

---

## 2. 핵심 설계 철학

Atlas는 처음부터 최종 biome 이름을 hard label로 저장하지 않는다.

대신 월드 환경을 다음 네 층으로 분리해 저장한다.

1. Climate Axes
2. Terrain Form
3. Terrain/Hydrology Overlays
4. Cover / Vegetation Structure

즉 atlas는 “이곳이 desert다”를 먼저 확정하는 계층이 아니라,
“이곳은 temperate 성향이 강하고, subhumid에 가깝고, plain form이며, riverine overlay가 약하게 있고, canopy potential이 높다”를 계산하는 계층이다.

최종 biome 이름은 atlas 이후의 resolver가 해석한다.

---

## 3. Atlas 좌표계

### 3.1 기본 단위
- 1 atlas cell = 256m × 256m
- 1 atlas cell = 16 × 16 chunk columns
- 1 atlas cell = 2 × 2 regions

### 3.2 좌표
- `AtlasCoord { x: i32, z: i32 }`

### 3.3 seed 원칙
- world seed + atlas coord = 항상 동일한 atlas 결과
- 생성 순서에 따라 결과가 달라지면 안 된다
- atlas는 chunk generation과 독립적으로 조회 가능해야 한다

---

## 4. Atlas의 책임과 비책임

## 4.1 책임
- ocean/land mask 계산
- continent structure 계산
- macro elevation 계산
- ridge / mountain field 계산
- river / basin / drainage field 계산
- temperature / humidity / inlandness 계산
- aridity / wetness / alpine / polar factor 계산
- thermal / moisture / overlay weights 계산
- ecotone strength 계산
- 디버그 맵 출력용 필드 제공

## 4.2 비책임
- 실제 블록 채우기
- 실제 표면 재질 결정
- 개별 식생/자원/POI 배치
- 개체 스폰
- 시간 기반 생태 시뮬레이션
- GPU 메쉬 생성

---

## 5. Atlas 데이터 계층

## 5.1 Raw Fields
Raw Fields는 atlas의 근본 환경 데이터다.

### 형태 / 위치
- `landness`
- `ocean_distance`
- `coast_distance`
- `continent_id`
- `continent_core_factor`

### 지형
- `macro_elevation`
- `slope`
- `ruggedness`
- `ridge_factor`
- `mountain_mass`
- `basinness`
- `pass_potential`

### 수계
- `river_source_potential`
- `river_flow_potential`
- `river_distance_estimate`
- `lake_potential`

### 기후
- `temperature`
- `humidity`
- `inlandness`

---

## 5.2 Derived Factors
Derived Factors는 raw field 조합으로 계산되는 해석 보조값이다.

- `aridity`
- `wetness`
- `polar_factor`
- `alpine_factor`
- `coast_factor`
- `wetland_factor`
- `riverine_factor`

예시:
- 낮은 humidity + 높은 inlandness → aridity 증가
- 높은 basinness + 높은 water access → wetness / wetland_factor 증가
- 높은 elevation + 높은 ridge_factor → alpine_factor 증가
- 매우 낮은 temperature → polar_factor 증가

---

## 5.3 Climate Axes
Atlas는 다음 두 개의 기후 축 weight를 저장한다.

### Thermal Axis
- `thermal.polar`
- `thermal.cold`
- `thermal.temperate`
- `thermal.warm`
- `thermal.hot`

### Moisture Axis
- `moisture.arid`
- `moisture.semi_arid`
- `moisture.subhumid`
- `moisture.humid`
- `moisture.wet`

이 축들은 부드러운 전이 구간을 가지며, hard bucket보다 weight 기반 저장을 우선한다.

---

## 5.4 Terrain Form
Atlas는 각 좌표의 기본 지형 형태를 다음 형태 계층으로 해석한다.

- `form.plain`
- `form.hill`
- `form.mountain`

이 값은 slope, ruggedness, ridge_factor, mountain_mass를 기반으로 계산한다.

원칙:
- plain은 기본 상태에 가깝다
- hill은 완만한 고저차와 불균일성이 있는 지형이다
- mountain은 명확한 산악권이며, 이동 장벽성과 강한 지형 구조성을 가진다

즉 mountain은 overlay가 아니라 terrain form에 속한다.

---

## 5.5 Terrain / Hydrology Overlays
Overlay는 기본 지형 형태 위에 덧씌워지는 특수 입지 성격이다.
Overlay는 서로 배타적이지 않으며 동시에 여러 개가 높아질 수 있다.

- `overlay.ocean`
- `overlay.coast`
- `overlay.riverine`
- `overlay.wetland`
- `overlay.alpine`

예:
- plain + coast
- plain + riverine
- plain + coast + wetland
- mountain + alpine

즉 overlay는 1택 axis가 아니라 중첩 가능한 location modifiers다.

---

## 5.6 Cover / Vegetation Structure
Atlas는 지표 피복과 개방도 성격도 함께 보존한다.

권장 방식은 hard class보다 potential/factor 저장이다.

- `cover.openness`
- `cover.grass_potential`
- `cover.shrub_potential`
- `cover.canopy_potential`
- `cover.forest_potential`

이 값은 temperature, humidity, aridity, wetness, form, overlay, drainage를 조합해 계산한다.

예:
- 높은 openness + 높은 grass_potential → 초원/평원 성향
- 높은 canopy_potential + 높은 forest_potential → 숲 성향
- 높은 aridity + 낮은 canopy_potential → 황량/저식생 성향

---

## 6. Atlas 처리 순서

Atlas는 대략 다음 순서로 계산한다.

1. ocean / continent structure
2. macro elevation
3. ridge / mountain chain field
4. basin / drainage / river potential
5. temperature field
6. humidity / inlandness
7. derived factors 계산
8. thermal axis weights 계산
9. moisture axis weights 계산
10. overlay axis weights 계산
11. ecotone strength 계산
12. debug map export

---

## 7. Ocean / Continent 구조

### 7.1 목표
- 바다는 중간 이상 비중으로 하나의 거대한 해양 시스템처럼 느껴져야 한다
- 그 위에 크고 작은 대륙, 섬, شبه대륙이 생성된다
- 플레이어는 바다를 건너 다른 대륙권이 있다는 감각을 가져야 한다

### 7.2 출력
- `landness`
- `ocean_distance`
- `coast_distance`
- `continent_id`
- `continent_core_factor`

### 7.3 주요 파라미터
- `ocean_coverage`
- `continent_scale_primary`
- `continent_scale_secondary`
- `coast_roughness`

### 7.4 추천 기본값
- `ocean_coverage = 0.58`
- `continent_scale_primary = 96 atlas cells`
- `continent_scale_secondary = 28 atlas cells`
- `coast_roughness = 0.35`

---

## 8. Mountain / Ridge 구조

### 8.1 목표
- 산은 월드 구조를 만드는 핵심 축이어야 한다
- 산 주변에는 작은 산, 험한 구릉, 절벽, 능선이 파생될 수 있어야 한다
- 일부 산맥은 이동 장벽 역할을 해야 한다

### 8.2 출력
- `ridge_factor`
- `mountain_mass`
- `ruggedness`
- `pass_potential`

### 8.3 주요 파라미터
- `ridge_continuity`
- `mountain_clustering`
- `pass_frequency`
- `hill_threshold`
- `mountain_threshold`
- `impassable_mountain_threshold`

### 8.4 추천 기본값
- `ridge_continuity = 0.78`
- `mountain_clustering = 0.68`
- `pass_frequency = 0.14`
- `hill_threshold = 0.42`
- `mountain_threshold = 0.68`
- `impassable_mountain_threshold = 0.86`

---

## 9. River / Lake / Wetland 구조

### 9.1 목표
- 강은 상류-중류-하류 성격 차이를 가지는 지형 축이어야 한다
- 산과 계곡에서 시작해 평야와 바다로 이어져야 한다
- 호수와 습지는 수계와 배수 구조의 결과로 나타나야 한다

### 9.2 출력
- `river_source_potential`
- `river_flow_potential`
- `river_distance_estimate`
- `lake_potential`
- `riverine_factor`
- `wetland_factor`

### 9.3 주요 파라미터
- `major_river_density`
- `tributary_density`
- `lake_basin_frequency`
- `wetland_threshold`

### 9.4 추천 기본값
- `major_river_density = 0.33`
- `tributary_density = 0.56`
- `lake_basin_frequency = 0.18`
- `wetland_threshold = 0.67`

---

## 10. Temperature / Humidity / Inlandness 구조

## 10.1 Temperature
Temperature는 대규모 온도 필드와 고도 보정을 결합해 계산한다.

입력:
- base temperature field
- macro elevation
- altitude lapse
- local climate continuity

출력:
- `temperature`
- `polar_factor`

추천값:
- `temperature_scale = 64 atlas cells`
- `altitude_lapse_strength = 0.46`
- `polar_temperature_threshold = 0.14`

---

## 10.2 Humidity
Humidity는 기본 습도장과 해양/수계/내륙성 보정을 결합한다.

입력:
- base humidity field
- ocean distance
- river / lake proximity
- inlandness
- basinness

출력:
- `humidity`
- `wetness`
- `aridity`

추천값:
- `humidity_scale = 48 atlas cells`
- `inland_dryness_strength = 0.62`
- `water_proximity_humidity_bonus = 0.28`

---

## 10.3 Alpine / Polar 분리
Alpine과 Polar는 같은 것이 아니다.

### Polar
- 매우 낮은 temperature가 주 원인
- 넓은 지역권으로 존재 가능
- 고도와 무관하게 형성 가능

### Alpine
- 높은 elevation + ridge/mountain 영향이 주 원인
- 주변이 온대라도 산 정상부만 형성 가능
- 지형 overlay 성격이 강함

출력:
- `polar_factor`
- `alpine_factor`

추천값:
- `alpine_threshold = 0.82`

---

## 11. Thermal / Moisture / Overlay Weights

## 11.1 Thermal Weights
Atlas는 temperature와 polar_factor를 바탕으로 다음 weight를 계산한다.

- `thermal.polar`
- `thermal.cold`
- `thermal.temperate`
- `thermal.warm`
- `thermal.hot`

여기서 중요한 것은 hard bucket이 아니라 부드러운 전이 구간을 갖는다는 점이다.

---

## 11.2 Moisture Weights
Atlas는 humidity, aridity, wetness를 바탕으로 다음 weight를 계산한다.

- `moisture.arid`
- `moisture.semi_arid`
- `moisture.subhumid`
- `moisture.humid`
- `moisture.wet`

즉 `hot + arid`, `cold + arid`, `temperate + humid` 같은 조합이 그대로 살아 있어야 한다.

---

## 11.3 Overlay Weights
Overlay는 base climate 위에 덧씌워지는 지형/수계/입지 성격이다.

- `overlay.ocean`
- `overlay.coast`
- `overlay.alpine`
- `overlay.wetland`
- `overlay.riverine`

즉 다음 같은 조합이 가능하다.

- temperate + humid + coast
- hot + arid + inland
- cold + semi_arid + alpine
- temperate + wet + riverine
- polar + coast

---

## 12. Ecotone

### 12.1 원칙
에코톤은 별도 biome ID가 아니다.  
두 개 이상의 axis/overlay 조합이 비슷하게 경쟁하는 transition band로 취급한다.

### 12.2 계산 감각
- thermal 경계
- moisture 경계
- overlay 경계
- coast / inland 전환
- wetland / upland 전환
- mountain / non-mountain 전환
이 겹치는 구간에서 ecotone_strength가 높아질 수 있다.

### 12.3 추천값
- `ecotone_band = 0.12`

---

## 13. Atlas가 직접 확정하지 않는 것

Atlas는 다음을 직접 확정하지 않는다.

- desert
- tundra
- temperate forest
- grassland
- cold steppe
- alpine rock
- coastal marsh

이런 이름은 atlas 이후의 `biome_resolver`가  
thermal / moisture / overlay / ruggedness / basinness 같은 조합을 읽고 해석한다.

예:
- `hot + arid + inland + low wetness` → desert 계열
- `cold + arid + non-polar` → cold steppe 계열
- `polar + flat + dry` → polar tundra 계열
- `temperate + humid + coast` → coastal temperate 계열
- `temperate + wet + basin + riverine` → wet lowland 계열

---

## 14. Debug Outputs

Atlas는 청크 생성 전 디버그 가능한 2D 맵을 제공해야 한다.

### 14.1 raw maps
- landness
- ocean distance
- coast distance
- elevation
- ridge
- ruggedness
- basinness
- river potential
- lake potential

### 14.2 climate maps
- temperature
- humidity
- inlandness
- aridity
- wetness
- polar factor
- alpine factor

### 14.3 axis maps
- thermal dominant class
- moisture dominant class
- overlay dominant class
- ecotone strength

### 14.4 목적
- 시드 검토
- 대륙/산맥/수계 스케일 튜닝
- climate belt 튜닝
- biome resolver 입력 검증
- 스폰 후보지 검토

---

## 15. Spawn Suitability

### 15.1 목표
- 첫 시작은 너무 극단적인 지형/기후를 피한다
- 시작 청크와 주변 소권역만 제어한다
- 일반 월드 규칙을 크게 깨지 않는다

### 15.2 추천 규칙
- thermal에서 temperate 우세
- moisture에서 subhumid~humid 우세
- overlay에서 alpine/polar/wetland/ocean 강한 셀 제외
- ridge core 제외
- 얕은 riverine, 낮은 ruggedness, 완만한 basin은 허용

### 15.3 추천값
- `spawn_safe_radius = 2 atlas cells`

---

## 16. Chunk Generation과의 연결

Chunk generation은 atlas에서 최소한 다음을 조회한다.

### raw / derived
- macro_elevation
- ridge_factor
- ruggedness
- basinness
- river_distance_estimate
- temperature
- humidity
- inlandness
- aridity
- wetness
- polar_factor
- alpine_factor

### axis / overlay
- thermal weights
- moisture weights
- overlay weights
- ecotone_strength

chunk layer는 이 값을 바탕으로:
- biome resolver 호출
- 실제 재질 선택
- 식생 규칙 선택
- 미세 서식지 계산
을 수행한다.

---

## 17. 불변식

1. Atlas는 실제 블록 데이터를 소유하지 않는다.
2. Atlas 출력은 seed + atlas coord만으로 재현 가능해야 한다.
3. Atlas는 final biome name이 아니라 환경 축과 overlay를 우선 보존한다.
4. Atlas는 디버그 가능한 2D 맵을 제공해야 한다.
5. Atlas와 biome resolver, chunk realization은 분리한다.