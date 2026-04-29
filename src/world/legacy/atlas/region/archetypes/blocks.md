# blocks

## Purpose

이 문서는 `RegionArchetype` 단계에서 **생태 적용 이전에 바닥으로 먼저 깔릴 수 있는 블럭들**을 정리한다.

지금 목표는 세밀한 식생 정의가 아니라, 지역별로 "여기엔 어떤 바닥 재질이 자연스러운가"를 아주 단순하게 잡는 것이다.

## Working Rule

- 이 단계의 블럭은 **base substrate** 성격이 강하다.
- 풀, 관목, 갈대, 이끼, 수생 식물 같은 생태 블럭은 여기서 본격적으로 다루지 않는다.
- 가능한 한 적은 종류로 시작하되, 아래 풀 안에서 필요하면 점진적으로 줄이거나 나눈다.
- 같은 블럭이라도 `wet`, `dry`, `snowy`, `frozen`, `bare` 같은 상태 변형이 나중에 붙을 수 있다.

---

## 1. Complete Candidate Base Pool

아래는 minimum / recommended keep / optional 을 합친 **전체 후보 풀**이다.

### Universal ground

- `grass`
- `dirt`
- `coarse_dirt`
- `sand`
- `gravel`
- `stone`
- `mud`
- `clay`
- `snow`
- `ice`
- `water`

### Wetland / flood / organic

- `peat`
- `silt`
- `humus`
- `leaf_litter`
- `moss`
- `podzol`
- `thin_soil`

### Dry / exposed / hardened ground

- `red_sand`
- `dry_grass`
- `wet_sand`
- `wet_gravel`
- `exposed_rock`
- `rock`
- `sandstone`

### Mountain / alpine / debris

- `scree`
- `alpine_soil`
- `moraine`

### Tropical / monsoon / weathered ground

- `jungle_grass`
- `laterite`

---

## 2. Very Simple Region-Family Mapping

아래는 "일단 최소한 이 정도는 깔릴 수 있다" 수준의 초안이다.

### Temperate / Grassland

- base: `grass`, `dirt`, `coarse_dirt`, `gravel`, `stone`
- wet pockets: `mud`, `clay`, `silt`
- richer soil pockets: `humus`, `leaf_litter`
- dry pockets: `coarse_dirt`, `gravel`

### Desert / Semi-Desert / Badlands

- base: `sand`, `gravel`, `stone`
- optional: `red_sand`, `sandstone`, `exposed_rock`, `rock`
- drain lines / dry washes: `gravel`, `sand`, `stone`
- very dry hardpan / transitional edges: `coarse_dirt`

### Savanna / Dry Shrubland

- base: `dry_grass`, `dirt`, `coarse_dirt`, `gravel`, `stone`
- dry hardpan patches: `gravel`, `exposed_rock`
- occasional richer soil pockets: `dirt`, `humus`

### Tropical Rainforest / Monsoon

- base: `jungle_grass`, `humus`, `leaf_litter`, `dirt`, `mud`, `clay`
- wet ground: `mud`, `clay`, `peat`
- stream-adjacent: `silt`, `mud`, `water`
- degraded / exposed spots: `dirt`, `laterite`

### Swamp / Marsh / Floodplain / Delta

- base: `mud`, `silt`, `clay`, `peat`
- waterlogged zones: `mud`, `water`
- levee / bar / edge patches: `sand`, `gravel`
- firm spots: `dirt`, `humus`

### Boreal / Taiga / Cold Wet Lowland

- base: `dirt`, `podzol`, `moss`, `gravel`, `stone`
- organic litter layer: `leaf_litter`, `humus`
- wet depressions: `mud`, `water`, `peat`
- seasonal snow cover: `snow`
- frozen patches: `ice`

### Tundra / Polar Barrens

- base: `dirt`, `gravel`, `stone`
- sparse organic layer: `moss`, `thin_soil`
- cold cover: `snow`, `ice`
- exposed wind-scoured ground: `gravel`, `stone`

### Coastal / Beach / Shore

- base: `sand`, `gravel`, `stone`
- wet intertidal strip: `wet_sand`, `wet_gravel`
- rocky shore: `rock`, `stone`, `gravel`
- lagoon / estuary mudflats: `mud`, `silt`, `clay`

### Mountain / Alpine / Glacial

- base: `stone`, `gravel`, `scree`
- thin soil pockets: `alpine_soil`, `dirt`, `thin_soil`
- cold cover: `snow`, `ice`
- exposed cliff / ridge: `stone`, `rock`, `exposed_rock`
- glacial debris: `moraine`

### Steppe / Plain / Basin / Plateau

- base: `grass`, `dirt`, `coarse_dirt`, `gravel`, `stone`
- dry patches: `coarse_dirt`, `gravel`
- basin wet pockets: `mud`, `clay`, `silt`
- richer pockets: `humus`

---

## 3. Archetype Family Notes

아래는 archetype 이름을 읽었을 때 바로 떠올릴 수 있는 1차 바닥 블럭 후보들이다.

### Launch candidates

#### `temperate_plain`
- `grass`, `dirt`, `coarse_dirt`, `gravel`, `stone`, `humus`, `leaf_litter`

#### `temperate_hills`
- `grass`, `dirt`, `coarse_dirt`, `gravel`, `stone`, `exposed_rock`, `humus`, `leaf_litter`

#### `temperate_plateau`
- `grass`, `dirt`, `gravel`, `stone`, `exposed_rock`, `coarse_dirt`

#### `steppe_plain`
- `grass`, `dry_grass`, `dirt`, `coarse_dirt`, `gravel`

#### `desert_plain`
- `sand`, `gravel`, `stone`, `red_sand`, `sandstone`, `exposed_rock`

#### `desert_dune_field`
- `sand`, `red_sand`, `gravel`

#### `sandy_beach_plain`
- `sand`, `wet_sand`, `gravel`, `stone`, `silt`

#### `coastal_cliffland`
- `stone`, `rock`, `gravel`, `sand`, `wet_gravel`

#### `cold_wet_lowland`
- `mud`, `clay`, `dirt`, `water`, `snow`, `peat`, `silt`

#### `tropical_rainforest_lowland`
- `jungle_grass`, `humus`, `leaf_litter`, `dirt`, `mud`, `clay`, `laterite`, `peat`

#### `tropical_rainforest_hills`
- `jungle_grass`, `humus`, `leaf_litter`, `dirt`, `mud`, `clay`, `stone`, `laterite`, `exposed_rock`

#### `glaciated_alpine`
- `stone`, `gravel`, `scree`, `snow`, `ice`, `moraine`

#### `tundra_plain`
- `dirt`, `gravel`, `stone`, `snow`, `ice`, `moss`, `thin_soil`

#### `oceanic_shelf`
- `sand`, `mud`, `silt`, `clay`, `stone`, `water`

### Extended candidates

#### `boreal_plain`
- `dirt`, `podzol`, `moss`, `leaf_litter`, `gravel`, `stone`, `snow`

#### `boreal_hills`
- `dirt`, `podzol`, `moss`, `leaf_litter`, `gravel`, `stone`, `snow`, `ice`

#### `boreal_wet_lowland`
- `mud`, `clay`, `podzol`, `moss`, `water`, `snow`, `peat`, `silt`

#### `swamp_lowland`
- `mud`, `peat`, `clay`, `silt`, `water`, `humus`

#### `marsh_floodplain`
- `mud`, `silt`, `clay`, `peat`, `water`, `sand`

#### `flooded_forest_floodplain`
- `mud`, `silt`, `clay`, `water`, `dirt`, `leaf_litter`, `humus`

#### `flooded_forest_alluvial_lowland`
- `mud`, `silt`, `clay`, `dirt`, `water`, `gravel`, `leaf_litter`, `humus`

#### `estuary_lowland`
- `mud`, `silt`, `clay`, `sand`, `water`, `peat`

#### `coastal_delta`
- `mud`, `silt`, `clay`, `sand`, `water`, `peat`

#### `monsoon_floodplain`
- `mud`, `clay`, `silt`, `dirt`, `water`, `humus`, `laterite`

#### `monsoon_delta`
- `mud`, `silt`, `clay`, `sand`, `water`, `peat`, `humus`

#### `savanna_plain`
- `dry_grass`, `grass`, `dirt`, `coarse_dirt`, `gravel`, `humus`

#### `savanna_hills`
- `dry_grass`, `grass`, `dirt`, `coarse_dirt`, `gravel`, `stone`, `exposed_rock`

#### `semi_desert_pediment`
- `gravel`, `stone`, `sand`, `exposed_rock`, `coarse_dirt`

#### `dry_shrubland_badlands`
- `stone`, `gravel`, `dirt`, `exposed_rock`, `sandstone`

#### `dry_shrubland_karst`
- `stone`, `gravel`, `exposed_rock`, `dirt`, `sandstone`

#### `mediterranean_shrubland_hills`
- `dirt`, `coarse_dirt`, `stone`, `gravel`, `exposed_rock`, `leaf_litter`

#### `temperate_rolling_plain`
- `grass`, `dirt`, `coarse_dirt`, `gravel`, `humus`

#### `temperate_basin`
- `grass`, `dirt`, `mud`, `clay`, `gravel`, `silt`, `humus`

#### `temperate_broad_valley`
- `grass`, `dirt`, `gravel`, `mud`, `clay`, `silt`, `leaf_litter`

#### `temperate_broadleaf_plain`
- `grass`, `dirt`, `humus`, `leaf_litter`, `coarse_dirt`, `gravel`

#### `temperate_mixed_hills`
- `grass`, `dirt`, `coarse_dirt`, `gravel`, `stone`, `leaf_litter`, `humus`

#### `temperate_escarpment_upland`
- `stone`, `gravel`, `exposed_rock`, `dirt`, `coarse_dirt`

#### `temperate_hills`
- `grass`, `dirt`, `coarse_dirt`, `gravel`, `stone`, `humus`, `leaf_litter`

#### `subalpine_wooded_front`
- `dirt`, `moss`, `gravel`, `stone`, `snow`, `leaf_litter`, `thin_soil`

#### `alpine_meadow_mountain`
- `grass`, `alpine_soil`, `stone`, `gravel`, `snow`, `scree`, `moraine`

#### `glacial_valley`
- `ice`, `snow`, `stone`, `gravel`, `moraine`, `scree`

#### `crevassed_icefield`
- `ice`, `snow`, `stone`, `scree`

#### `polar_barrens_plain`
- `stone`, `gravel`, `snow`, `ice`, `thin_soil`

#### `fjord_coast`
- `stone`, `rock`, `gravel`, `sand`, `ice`, `wet_gravel`

#### `barrier_coast`
- `sand`, `wet_sand`, `gravel`, `water`, `clay`, `silt`

#### `lagoon_coast`
- `sand`, `mud`, `silt`, `clay`, `water`, `peat`

#### `rocky_shore_coast`
- `stone`, `rock`, `gravel`, `sand`, `wet_gravel`

#### `mangrove_lagoon`
- `mud`, `silt`, `clay`, `water`, `peat`, `humus`

#### `mangrove_delta`
- `mud`, `silt`, `clay`, `water`, `peat`, `humus`

#### `desert_basin`
- `sand`, `gravel`, `clay`, `stone`, `red_sand`

#### `desert_alluvial_fan`
- `gravel`, `sand`, `stone`, `clay`, `silt`

#### `desert_mesa_country`
- `stone`, `gravel`, `sand`, `exposed_rock`, `sandstone`

#### `boreal_ridge_country`
- `stone`, `gravel`, `snow`, `ice`, `moss`, `scree`

#### `monsoon_plateau`
- `dirt`, `clay`, `stone`, `humus`, `gravel`, `laterite`

#### `alpine_ravine_country`
- `stone`, `gravel`, `scree`, `snow`, `ice`

---

## 4. Non-Goals for Now

- 식생 블럭 구체화
- 블럭 상태 전이 규칙 구체화
- 계절별 변형 전부 정의
- 지역별 블럭 팔레트의 최종 확정
- material/render layer 이름과 1:1 고정

이 문서는 지금은 **최소 분류를 먼저 잡기 위한 작업 메모**다.
구현이 가까워지면 archetype별로 더 줄이고, 중복 블럭은 합치고, 실제 렌더/머티리얼 레이어에 맞춰 다시 다듬는다.
