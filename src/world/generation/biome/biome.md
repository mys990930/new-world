# biome

## 역할

`biome`은 새 graph-first 생성 파이프라인에서 final cell biome context와 biome 판정을 소유한다.

이 모듈은 가벼운 정책 계층이다. Voronoi graph와 macro map이 확정한 기후, macro elevation,
continentality, coast exposure, mountain context, ruggedness, water role을 읽고, 이후 `macro_field`,
`surface_plan`, `voxel` 단계가 소비할 안정적인 biome class를 결정한다.

## 책임

- graph-first final cell biome context 정의
- legacy atlas region cell에 의존하지 않는 graph-first biome enum 정의
- ocean biome을 `ShallowOcean`과 `DeepOcean`으로 분리
- ocean, coast, lake, wetland, dry basin, 일반 land climate biome 판정
- `continentality`로 내륙성 biome과 해안/해양성 biome 구분
- `ruggedness`로 rocky coast, dry shrubland, alpine/mountain 성격 구분
- 실제 `graph_voronoi_preview` 기본 footprint의 절대 입력 분포를 기준으로 threshold 유지

## 비책임

- Voronoi graph 생성
- macro ownership 또는 elevation resolve
- hydrology routing
- material 선택
- voxel fill
- preview binary rendering

## 불변식

1. water/coast/wetland role은 climate-only land 판정보다 우선한다.
2. ocean은 반드시 `ShallowOcean`과 `DeepOcean`으로 나뉜다.
3. classifier는 명시적인 `GraphBiomeContext` 값만 읽고 deterministic하게 동작한다.
4. `GraphBiomeContext`는 `basinness`를 들고 있지 않다. 폐쇄분지 의미는 biome 판정 전에
   `GraphBiomeWaterRole::DryBasin`으로만 전달된다.
5. `GraphBiomeWaterRole::Lake`는 climate, elevation, hydration으로 추론하지 않는다.
   `MacroSurfaceKind::LakeCandidate`가 lake cell의 source of truth이며, biome classifier는 이를
   `GraphBiomeKind::Lake`로 1:1 보존한다.
6. legacy `RegionClassCell`이나 atlas archetype은 이 계약에 포함되지 않는다.

## 관측된 입력 분포

아래 값은 `graph_voronoi_preview` 기본 footprint와 같은 조건에서 seed `1..=128`, center `(0, 0)`,
`3840x2160`, `world_span_blocks = 32768`로 visible site center cell `2,096,468`개를 샘플한 값이다.
색상 preview는 각 field의 절대 `0..1` 값을 쓰지만, 실제 생성값은 전체 range를 다 쓰지 않는다.
따라서 biome threshold도 이 관측 분포를 기준으로 잡는다.

| field | p0 | p5 | p25 | p50 | p75 | p90 | p95 | p99 | p100 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| temperature | 0.333 | 0.447 | 0.508 | 0.554 | 0.601 | 0.641 | 0.662 | 0.698 | 0.768 |
| hydration | 0.133 | 0.337 | 0.427 | 0.496 | 0.567 | 0.626 | 0.657 | 0.709 | 0.830 |
| effective_temperature | 0.210 | 0.403 | 0.474 | 0.524 | 0.574 | 0.617 | 0.641 | 0.681 | 0.760 |
| elevation | -0.440 | -0.181 | 0.017 | 0.351 | 0.426 | 0.483 | 0.516 | 0.576 | 0.744 |
| continentality | -0.565 | -0.175 | 0.004 | 0.137 | 0.272 | 0.390 | 0.455 | 0.564 | 0.837 |
| coastness | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.500 | 1.000 | 1.000 | 1.000 |
| mountainness | 0.000 | 0.000 | 0.000 | 0.004 | 0.089 | 0.211 | 0.291 | 0.442 | 0.840 |
| ruggedness | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.020 | 0.067 | 0.204 | 0.734 |

중요한 해석:

- `temperature >= 0.68`은 전체 상위 1% 안쪽이므로 tropical gate로 쓰기에는 너무 좁다.
  현재 hot/tropical 기준은 `effective_temperature >= 0.62`로 둔다.
- `hydration >= 0.76`은 극단적으로 드물다. 매우 습한 forest 기준은 `0.68` 전후로 둔다.
- land `continentality`의 중앙값은 약 `0.21`이고, `0.30` 이상은 상당히 내륙성이다.
  savanna/desert류는 보통 `0.24` 이상을 요구한다.
- coast role의 `coastness`는 현재 거의 `1.0`으로 들어온다. lagoon을 `coastness < 1.0`으로
  제한하면 실제로 사라지므로, 낮고 평평하고 매우 습한 coast cell 중 narrow pocket으로 제한한다.
- coast role의 `ruggedness/mountainness`는 대부분 `0`이다. rocky coast는 절대값이 작아도
  coast subset 안에서 non-zero relief를 가진 cell을 잡는다.
- alpine은 이 스케일에서 `elevation >= 0.56`, `mountainness >= 0.34`, `ruggedness >= 0.10`을
  함께 만족해야 한다. 이 값들은 land 분포의 상단부를 겨냥한다.

## 공통 입력과 보정

`GraphBiomeContext` 입력:

- `temperature`: final cell temperature, `0.0..=1.0`
- `hydration`: final cell hydration, `0.0..=1.0`
- `elevation`: signed macro elevation
- `continentality`: 해양성/대륙성 context, `-1.0..=1.0`
- `coastness`: ocean coast 영향도, `0.0..=1.0`
- `mountainness`: 산악 context, `0.0..=1.0`
- `ruggedness`: 거친 relief context, `0.0..=1.0`
- `water_role`: `Land`, `Coast`, `ShallowOcean`, `DeepOcean`, `Lake`, `Wetland`, `DryBasin`

모든 입력은 `GraphBiomeContext::clamped`를 거쳐 판정된다.

판정용 온도는 아래처럼 계산한다.

```text
effective_temperature =
    clamp01(temperature - mountainness * 0.14 - ruggedness * 0.04 - max(elevation, 0.0) * 0.08)
```

의미: 같은 base temperature라도 높고, 산악적이고, 거친 지형은 실제 생태대에서 더 차갑게 동작한다.
다만 source climate field를 뒤집지 않도록 보정 폭은 작게 유지한다.

## Water Role 우선순위

- `DeepOcean` -> `DeepOcean`
- `ShallowOcean` -> `ShallowOcean`
- `Lake` -> `Lake`
- `Coast` -> coast 전용 판정
- `Wetland` -> wetland 전용 판정
- `DryBasin` -> dry/arid 판정
- `Land` -> alpine/cold/dry/tropical/temperate 순서로 climate 판정

## Coast 판정

Coast는 해안 전이대다. 일반적인 해안은 `SandyCoast`가 가장 많아야 하고, 그 다음 흔한 변형이
`RockyCoast`다. `Mangrove`, `EstuarineCoast`, `LagoonCoast`는 특수 조건이다.

`GraphBiomeWaterRole::Coast`는 아래 순서로 판정한다.

1. `RockyCoast`
   - 기준: `ruggedness > 0.0 || mountainness > 0.0 || elevation >= 0.08`
   - 이유: 현재 coast subset에서는 rugged/mountain 값이 대부분 `0`이다. 따라서 절대 threshold를 높게
     두면 rocky coast가 사라진다. coast 안에서 non-zero relief가 있는 cell을 암석/절벽성 해안으로 본다.
2. `Mangrove`
   - 기준: `effective_temperature >= 0.62`, `hydration >= 0.66`, `coastness >= 0.98`,
     `ruggedness < 0.004`, `continentality <= 0.06`
   - 이유: 맹그로브는 hot, very wet, smooth, oceanic shoreline이어야 한다. 거칠거나 내륙성이 강한
     cell은 제외한다.
3. `LagoonCoast`
   - 기준: `coastness >= 0.98`, `elevation <= 0.06`, `hydration >= 0.68`,
     `ruggedness <= 0.002`, `mountainness <= 0.005`, `0.035 <= continentality <= 0.080`
   - 이유: lagoon은 shoreline 전체가 아니라 낮고 평평한 아주 습한 coastal pocket이다. 현재 context상
     coast role은 coastness가 거의 `1.0`이므로, continentality를 좁은 band로 제한해 shoreline 안쪽
     pocket 느낌을 준다.
4. `EstuarineCoast`
   - 기준: `hydration >= 0.67`, `coastness >= 0.98`, `ruggedness < 0.008`,
     `continentality <= 0.07`
   - 이유: 하구/조석 해안은 매우 습하고 완만한 해안이다. lagoon보다 넓지만, 여전히 ordinary sandy
     coast보다 특수하다.
5. `SandyCoast`
   - 기준: coast fallback
   - 이유: smooth하고 특수 조건이 없는 일반 해안은 모래/자갈 해안으로 둔다.

## Wetland 판정

Wetland는 macro/hydrology가 이미 습지성 role로 넘긴 cell이다. biome classifier는 숲이 가능한지,
따뜻한 늪인지, 열린 습지인지만 나눈다.

1. `FloodedForest`
   - 기준: `hydration >= 0.68`, `effective_temperature >= 0.50`, `mountainness < 0.48`,
     `ruggedness < 0.36`, `continentality >= -0.20`
   - 이유: flooded forest는 매우 습하고 충분히 따뜻하며, 숲이 설 수 있는 낮은 relief의 범람원이다.
2. `Swamp`
   - 기준: `effective_temperature >= 0.50`, `hydration >= 0.58`, `ruggedness < 0.40`
   - 이유: swamp는 warm/wet/smooth wetland다. flooded forest보다 숲 조건은 약하지만 차갑고 열린 marsh와는 구분한다.
3. `Marsh`
   - 기준: wetland fallback
   - 이유: 더 차갑거나 숲이 성립하기 애매한 열린 습지는 marsh다.

## Dry / Arid 판정

`DryBasin`이거나 일반 land에서 `hydration < 0.42`이면 dry/arid branch를 탄다.
내륙 건조 biome은 보통 `continentality` 또는 낮은 `coastness`를 요구한다. 단, `DryBasin` role은
이미 폐쇄분지 의미를 갖고 있으므로 continentality가 음수여도 dry branch의 내륙성 조건을 통과한다.

1. `PolarBarrens`
   - 기준: `effective_temperature <= 0.34`
   - 이유: 건조 branch에 들어왔더라도 너무 차가우면 사막보다 한랭 황무지가 맞다.
2. `Desert`
   - 기준: `hydration < 0.30`, `effective_temperature >= 0.60`,
     `is_inland_dry_context(0.24)`, `coastness <= 0.45`
   - 이유: 실제 hot desert는 매우 건조하고 덥고 내륙성이 강하다. 관측 hydration p1이 약 `0.28`이므로
     `0.30`은 드문 극건조 조건이다.
3. `DryShrubland`
   - 기준: `ruggedness >= 0.12`, `hydration < 0.42`, `is_inland_dry_context(0.10)`
   - 이유: 거칠고 건조한 구릉/사면은 연속 초원보다 관목지로 읽힌다. ruggedness는 전체 p95보다 높은
     relief를 잡는다.
4. `SemiDesert`
   - 기준: `hydration < 0.36`, `effective_temperature >= 0.52`, `is_inland_dry_context(0.18)`
   - 이유: desert보다는 덜 극단적이지만 여전히 건조하고 따뜻한 내륙/비해안 지형이다.
5. `Steppe`
   - 기준: `effective_temperature <= 0.56`, `is_inland_dry_context(0.18)`
   - 이유: steppe는 비교적 서늘하거나 온난한 대륙성 건조 초원이다.
6. `MediterraneanShrubland`
   - 기준: `0.50 <= effective_temperature <= 0.64`, `0.36 <= hydration <= 0.48`,
     `coastness >= 0.18`, `continentality <= 0.30`, `ruggedness < 0.12`
   - 이유: 지중해성 관목지는 완전 내륙 사막이 아니라 mild, seasonally dry, near-coast 조건이다.
7. `Savanna`
   - 기준: savanna 조건을 만족할 때
   - 이유: savanna는 hot/dry만이 아니라 내륙성 open woodland/grassland다.
8. `TemperateGrassland`
   - 기준: dry/arid fallback
   - 이유: 위 특수 건조 biome이 아니면 온대/냉온대 건조 초원으로 둔다.

`is_inland_dry_context(x)`:

```text
water_role == DryBasin || continentality >= x || coastness <= 0.35
```

Savanna 조건:

```text
effective_temperature >= 0.62
0.38 <= hydration <= 0.52
continentality >= 0.24
coastness <= 0.35
ruggedness < 0.18
```

의미: savanna는 뜨겁고 계절적으로 건조한 열린 내륙 지형이다. coast 영향이 크면 tropical dry forest나
coast 계열이 더 적절하다.

## Cold / Alpine 판정

High alpine gate:

```text
elevation >= 0.56 && mountainness >= 0.34 && ruggedness >= 0.10
```

의미: 현재 관측 분포에서 elevation `0.56`은 land 상위권, mountainness `0.34`는 land p95 근처,
ruggedness `0.10`은 land p95보다 높은 relief다. 높기만 한 smooth plateau가 alpine으로 바뀌지 않도록
세 값을 모두 요구한다.

Alpine branch:

1. `PolarIce`
   - 기준: `elevation >= 0.68`, `effective_temperature <= 0.28`
   - 이유: 매우 높고 충분히 차가운 산지는 빙설로 둔다.
2. `PolarBarrens` 또는 `Tundra`
   - 기준: `effective_temperature <= 0.34`, 또는
     `ruggedness >= 0.24 && effective_temperature <= 0.44 && hydration < 0.44`
   - 세부: `hydration < 0.36`이면 `PolarBarrens`, 아니면 `Tundra`
   - 이유: 매우 차갑거나 바람에 깎인 거친 고산 건조지는 숲/초지가 아니라 한랭 황무지/툰드라다.
3. `SubalpineWoodland`
   - 기준: `effective_temperature <= 0.50`, `hydration >= 0.50`
   - 이유: alpine gate를 통과했지만 충분히 습하고 아주 춥지는 않은 산악 경계림이다.
4. `AlpineMeadow`
   - 기준: alpine fallback
   - 이유: 높고 산악적이고 거칠지만 빙설/툰드라/아고산림 조건이 아니면 고산 초지다.

Non-alpine cold land:

1. `PolarIce`
   - 기준: `effective_temperature <= 0.24`
   - 이유: 현재 분포에서는 극히 드문 최저온 구간이다.
2. `PolarBarrens`
   - 기준: `effective_temperature <= 0.34`, `hydration < 0.36`
   - 이유: 차갑고 건조하면 숲이 아니라 한랭 황무지다.
3. `Tundra`
   - 기준: `effective_temperature <= 0.38`
   - 이유: 숲이 성립하기 어려운 열린 한랭 지형이다.
4. `BorealForest`
   - 기준: `effective_temperature <= 0.46`, `hydration >= 0.44`
   - 이유: 차갑지만 충분히 습하면 침엽수림/타이가가 된다.
5. `Steppe`
   - 기준: `effective_temperature <= 0.46`, `hydration < 0.44`
   - 이유: 차갑고 건조하면 boreal forest보다 steppe가 맞다.

## Tropical 판정

일반 land에서 dry/arid와 alpine/cold를 통과한 뒤 `effective_temperature >= 0.62`이면 tropical branch를 탄다.
`0.62`는 전체 effective temperature p90 근처라, 기존 `0.68`처럼 tropical biome을 상위 1%에만 가두지 않는다.

1. `TropicalRainforest`
   - 기준: `hydration >= 0.68`
   - 이유: hot + very wet. hydration p95~p99 사이의 습윤한 열대림이다.
2. `MonsoonForest`
   - 기준: `hydration >= 0.60`
   - 이유: hot + wet이지만 rainforest보다 덜 극단적인 계절성 숲이다.
3. `Savanna`
   - 기준: savanna 조건
   - 이유: hot + moderate hydration이어도 내륙성/open 조건을 만족해야 savanna다.
4. `TropicalDryForest`
   - 기준: `hydration >= 0.44`, 또는 tropical fallback
   - 이유: 덥지만 savanna처럼 충분히 내륙 개방 조건이 아니면 건조 열대림으로 둔다.

## Temperate 판정

위 branch에 걸리지 않은 land는 hydration으로 나눈다.

1. `TemperateRainforest`
   - 기준: `hydration >= 0.68`
   - 이유: 온대에서도 관측 p95 이상에 가까운 매우 습한 cell은 rainforest다.
2. `TemperateMixedForest`
   - 기준: `hydration >= 0.56`
   - 이유: land hydration p75 근처 이상의 충분히 습한 온대 혼합림이다.
3. `TemperateBroadleafForest`
   - 기준: `hydration >= 0.46`
   - 이유: 중간 습윤도의 온대 활엽수림이다.
4. `TemperateGrassland`
   - 기준: temperate fallback
   - 이유: 숲을 유지할 만큼 습하지 않은 온대 초원/평원이다.

## 현재 기본 footprint 결과

위 기준으로 seed `1..=128`, 기본 graph-voronoi preview footprint에서 모든 세분화 biome이 적어도 한 번
이상 등장한다. 큰 비율은 `ShallowOcean`, `SandyCoast`, `TemperateBroadleafForest`,
`TemperateGrassland`, `TemperateMixedForest`, `BorealForest`, `Steppe`가 차지하고, `LagoonCoast`,
`Mangrove`, `PolarIce`, `TropicalRainforest`는 의도적으로 rare class로 남는다.
