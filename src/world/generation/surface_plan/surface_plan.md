# surface_plan

## 역할

`surface_plan`은 graph-first generator의 14단계인 biome/material/water/coast surface policy resolve
계약을 소유한다.

이 단계는 stage 13 `heightfield`가 확정한 column height/water hint와, stage 8 final cell context에서
이미 resolve된 biome/context를 함께 읽어 마지막 `voxel` fill이 소비할 column별 block-stack plan을 만든다.

이 단계는 visible material boundary를 안정적으로 만들지만, hard polygon owner를 그대로 색칠하지
않는다.

---

## 책임

- 이미 resolve된 biome influence와 hydrology role에서 surface policy 선택
- ocean, coast, lake, river, wetland, floodplain의 material 의미 구분
- elevation, hydration, hydrology role에 따른 beach/soil/sediment/vegetation 전환
- column voxel fill이 읽을 surface column plan 정의
- seasonal/runtime surface state와 연결될 수 있는 world-side 계약 유지
- biome별 기본 block palette와 hydrology override priority 정의
- deterministic material transition과 block-scale boundary breakup 정의

---

## 비책임

- graph construction
- hydrology solve
- final height 계산
- block storage mutation
- renderer material upload
- biome 재분류
- river/lake/ocean topology 재해석
- vegetation blueprint 또는 placement 생성

---

## 입력 계약

surface resolve는 아래 입력을 함께 본다.

- stage 8 final cell context의 continuous temperature / hydration / ruggedness
- dominant site와 blended biome influence
- heightfield surface y
- ocean/lake/river/wetland/coast role
- river flow accumulation과 floodplain width
- ridge/fault/cliff guide
- macro_field/pixelize/heightfield가 보존한 meso feature material hint와 baked contribution mask
- local soil/sediment class
- runtime season/weather state가 허용하는 override

현재 graph-first 구현에서는 canonical noisy boundary owner의 biome/context가 `MacroFieldSample`에
보존되고, height/water/river bed hint는 `HeightfieldColumn`에 보존된다. `surface_plan`의 첫 구현은 이
둘을 같은 row-major column footprint에서 받아 하나의 입력으로 묶는다. `MacroFieldSample.nearest_site`,
water/mask channel, material policy가 읽는 `biome`/`biome_context`는 같은 noisy owner region을 따라야
한다.

```text
SurfaceColumnInput {
  world_x,
  world_z,
  heightfield: HeightfieldColumn,
  biome: Option<GraphBiomeKind>,
  biome_context: Option<GraphBiomeContext>,
  runtime_surface: Option<SurfaceCondition>,
}
```

`surface_plan`은 이 입력을 만들기 위해 graph, macro map, hydrology, river plan, boundary를 다시 query하지
않는다. upstream cache가 보존한 scalar/channel만 소비한다.

초기 area-level API는 아래 방향을 목표로 한다.

```text
generate_surface_plan_area(
  heightfield: &HeightfieldTile,
  macro_field: Option<&MacroFieldTile>,
  config: SurfacePlanConfig,
) -> SurfacePlanArea
```

`MacroFieldTile`은 biome/context handoff가 heightfield 또는 pixelize output으로 내려오기 전까지의
compatibility source다. 장기적으로는 `HeightfieldColumn` 또는 downstream column cache가 biome/context를
직접 보존해야 한다.

---

## 출력 계약

surface plan은 `ChunkData`를 직접 수정하지 않고, voxel fill이 읽을 column별 결정을 만든다.

```text
SurfaceColumnPlan {
  world_x,
  world_z,
  surface_y,
  water_y,
  hydrology_role,
  top_block,
  subsurface_block,
  base_block,
  underwater_top_block,
  exposed_block,
  sediment_block,
  soil_depth_blocks,
  vegetation_allowed,
  cover_phase,
}
```

`top_block`, `subsurface_block`, `base_block`, `underwater_top_block`, `exposed_block`,
`sediment_block`은 registry key 문자열이나 이미 resolve된 block id로 표현할 수 있다. cache/hot path에서는
`BlockId`가 유리하지만, 정책 테스트와 문서화 단계에서는 block key가 더 읽기 쉽다.

`soil_depth_blocks`는 voxel fill이 surface 아래 몇 block을 subsurface로 채울지 결정하는 값이다.
그 아래는 `base_block`을 사용한다. 물 column에서는 terrain bed 위의 top material과 water material이
분리되어야 하므로, `water_y`가 있더라도 `surface_y` bed material은 계속 보존한다.

---

## Resolve 순서

surface resolve는 아래 priority를 고정한다.

1. `HeightfieldColumn`의 `terrain_kind`, water hint, macro mask를 읽어 hydrology surface role을 정한다.
   - `Ocean`, `Lake`, `River`, `Wetland`, `Coast`, `DryBasin`, `Ridge`, `Land`를 구분한다.
   - ocean, lake, river, wetland, coast는 같은 water mask로 합치지 않는다.
   - river는 selected hydrology를 다시 풀지 않고 `river_valley_strength`, `river_flow_hint`,
     `river_bed_depth_blocks`, `river_gravel_hint`, `river_cutbank_hint`만 읽는다.
   - `coast_mask`는 넓은 coast distance diagnostic이므로 그 자체만으로 `Coast` role을 만들지 않는다.
     coast material은 explicit coast terrain, stage 8 coast water role, 또는 coast biome에서 온다.
2. biome과 final context를 읽어 기본 material policy family를 고른다.
   - biome은 이미 stage 8에서 resolve된 `GraphBiomeKind`를 소비한다.
   - `surface_plan`은 temperature/hydration으로 biome을 다시 판정하지 않는다.
3. hydrology override를 적용한다.
   - active ocean/lake/river bed, saturated wetland, strong floodplain, shoreline/intertidal strip은
     biome 기본 top보다 우선한다.
   - dry basin은 water override가 아니라 closed lowland/dry sediment override다.
4. deterministic transition을 적용한다.
   - visible material boundary는 hard owner line을 그대로 따르지 않는다.
   - transition은 world-space coordinate, seed/generator version, source material pair에서 결정되는
     stable noise 또는 boundary-step pass를 사용한다.
   - 이 noise는 biome이나 hydrology role을 새로 만들지 않고, 이미 선택된 policy family 안의 top/sediment
     후보만 흔든다.

---

## Hydrology Override

hydrology role은 biome 기본 palette보다 강하지만, 모든 주변 땅을 mud로 바꾸지는 않는다.

- active ocean water가 있는 column은 bed top을 `sand`, `silt`, `clay`, `gravel`, `stone` 계열로 선택한다.
  shallow/low-energy shelf는 `sand`/`silt`, higher-energy bed는 `gravel` 쪽으로 갈 수 있다.
- lake column은 `silt`, `clay`, `mud`를 기본 sediment로 쓰고, rugged edge나 inlet/outlet 근처에서는
  `gravel`을 허용한다.
- river core는 `gravel`, `silt`, `mud`, `wet_gravel` 계열을 쓴다. flow와 gravel/cutbank hint가 높을수록
  `gravel`/`wet_gravel`, low-energy floodplain은 `silt`/`mud`/`clay` 쪽으로 간다.
- river valley/fan scalar가 남아 있는 dry land는 그 자체만으로 연속 sediment patch가 되면 안 된다.
  dry bank나 mouth shoulder material은 selected river의 bed/roughness/gravel/cutbank hint와 충분히 강한
  valley/flow hint를 함께 통과한 column에서만 deterministic speckle로 나타난다. active river core와
  water column의 bed material은 계속 river sediment가 우선한다.
- wetland는 `mud`, `peat`, `silt`, `clay`를 우선한다.
- coast는 sandy/estuarine/mangrove 계열 material을 우선한다.
  ordinary coast는 `sand`/`wet_sand`, estuary/lagoon/mangrove는 active water, explicit coast terrain, or
  already-coast columns with strong coast mask에서는 `mud`/`silt`/`clay`/`peat`를 우선한다.
  `coast_mask`만 있는 ordinary land는 sand coast로 승격하지 않는다. `RockyCoast`, ruggedness, ridge scalar는
  현재 기본 coast material을 `gravel`/`wet_gravel`/`rock`으로 승격하지 않는다.
  dry weak coast-mouth shoulder는 selected mouth hint가 있을 때만 local `mud`/`silt` speckle을 허용하고,
  나머지는 sandy/wet-sandy transition으로 남겨 broad rectangular mouth patch를 만들지 않는다.
- dry basin은 `clay`, `silt`, `thin_soil`, `coarse_dirt`, `sand` 같은 dry sediment를 쓰며 water column을
  새로 만들지 않는다.

---

## Material Transition

visible material boundary는 hard owner 경계를 그대로 따라가면 안 된다.

권장 방식:

- continuous field gradient
- domain-warped boundary distance
- deterministic dithering
- cover override
- hydrology role 우선순위
- slope/exposure 기반 rocky override는 future work다. 현재 기본 resolve에서는 비활성이다.

launch 구현의 material resolve는 `MacroFieldTile` metadata가 있는 area path에서
`MacroFieldSample.nearest_site`를 surface material cell 기준으로 사용한다. 이 값은 raw nearest graph
cell이 아니라 canonical noisy boundary를 따른 visible owner다. 먼저 column별 hydrology/biome policy를
계산하되, 최종 pass에서 active water column(`water_y.is_some()`)만 제외하고 non-water surface top은 같은
noisy owner site 안에서 하나의 canonical top block으로 정규화한다. dry river bank, dry lake/ocean
fringe처럼 hydrology role은 남아 있지만 실제 물 column이 아닌 표면도 정규화 대상이다. 따라서 한 noisy
Voronoi owner cell 내부의 일반 표면이 `sand`/`wet_sand`나 `grass`/`sand`처럼 넓은 덩어리로 갈라지면
회귀다.

owner 내부 정규화가 끝난 뒤에는 작은 final material-only boundary wiggle pass를 한 번 더 적용한다. 이
pass는 인접 owner/material 경계의 비활성 non-water surface column 사이에서만 orthogonal neighbor의
visual material fields(`top_block`과 대응되는 visual companion block fields)를 deterministic하게 복사할 수
있다. 이 단계는 owner site, biome, `surface_y`, `water_y`, `hydrology_role`을 바꾸지 않으며 active water
column 경계는 넘지 않는다. 서로 맞닿은 두 owner column이 서로의 base top material을 맞교환하는 경우는
checkerboard artifact를 피하기 위해 base material로 되돌린다.

`boundary_mix_radius_blocks`는 macro owner metadata가 없는 fallback 또는 실제 water bed 같은 protected column을
위한 보조 pass로 남아 있지만, macro-field 기반 preview/generation path에서는 noisy-owner 정규화가 최종
owner interior material을 먼저 닫은 뒤 shared boundary 주변만 visual-only로 다시 흔든다. owner 내부
mix는 서로 다른 owner site 사이의 material copy를 계속 금지하고, final boundary wiggle만 제한적으로 owner
경계를 넘는다.

---

## Biome Material Policy

graph-first biome별 material policy는 legacy atlas archetype을 그대로 복사하지 않는다. 기존
`legacy/surface/material.rs`의 `MaterialPolicyDef`와
`legacy/atlas/region/archetypes/blocks.md`의 block 후보 pool을 migration reference로 삼아,
`GraphBiomeKind`별 launch palette를 별도로 정의한다.

초기 policy는 아래 field를 갖는 작은 table로 충분하다.

```text
BiomeSurfacePolicy {
  biome,
  family,
  default_top,
  dry_top,
  wet_top,
  frozen_top,
  subsurface,
  base,
  exposed,
  sediment,
}
```

이 table은 biome 의미의 기본값만 제공한다. 실제 column output은 hydrology override와 deterministic
transition을 거친 뒤 확정된다.

초기 graph-first mapping은 아래를 seed table로 사용한다. `family`는 구현상의 policy family 이름이며,
legacy `MaterialPolicyId`와 1:1로 고정되지 않는다.

| biome | family | default top | subsurface | base | wet | exposed | sediment | note |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `ShallowOcean` | `OceanicShelf` | `sand` | `silt` | `stone` | `silt` | `stone` | `silt` | water fill은 hydrology role이 결정한다. |
| `DeepOcean` | `DeepOceanFloor` | `silt` | `clay` | `stone` | `silt` | `stone` | `silt` | legacy에는 shelf만 있으므로 deep floor family를 새로 둔다. |
| `Mangrove` | `MangroveMudflat` | `mud` | `peat` | `dirt` | `mud` | `humus` | `silt` | coast/wetland hybrid다. |
| `EstuarineCoast` | `EstuarineMudflat` | `mud` | `silt` | `clay` | `mud` | `wet_sand` | `silt` | ordinary beach보다 river/ocean mixing sediment가 우선한다. |
| `LagoonCoast` | `LagoonMudflat` | `mud` | `silt` | `clay` | `wet_sand` | `sand` | `silt` | shore edge는 sand/wet sand, lagoon interior는 mud/silt 쪽으로 간다. |
| `RockyCoast` | `CoastalCliff` | `gravel` | `gravel` | `stone` | `wet_gravel` | `rock` | `wet_gravel` | legacy coastal cliff policy와 직접 대응한다. |
| `SandyCoast` | `SandyBeach` | `sand` | `sand` | `stone` | `wet_sand` | `stone` | `silt` | legacy sandy beach policy와 직접 대응한다. |
| `Lake` | `InlandLakeShore` | `silt` | `clay` | `stone` | `mud` | `gravel` | `silt` | ocean/coast와 분리된 inland standing-water policy다. |
| `Marsh` | `ColdWetland` | `mud` | `clay` | `dirt` | `peat` | `gravel` | `silt` | legacy wetland policy와 직접 대응한다. |
| `Swamp` | `WarmSwamp` | `peat` | `clay` | `dirt` | `mud` | `humus` | `silt` | warm/organic wetland로 `ColdWetland`보다 peat/humus 성격이 강하다. |
| `FloodedForest` | `FloodedForestWetland` | `mud` | `silt` | `dirt` | `peat` | `humus` | `silt` | swamp와 겹치지만 forestable firm spot을 남긴다. |
| `Desert` | `DesertSurface` | `sand` | `sand` | `sandstone` | `wet_sand` | `exposed_rock` | `gravel` | legacy desert policy와 직접 대응한다. |
| `SemiDesert` | `SemiDesertTransition` | `red_sand` | `coarse_dirt` | `sandstone` | `wet_sand` | `exposed_rock` | `gravel` | desert/steppe 사이 transition family다. |
| `Steppe` | `SteppeGrassland` | `dry_grass` | `dirt` | `stone` | `mud` | `gravel` | `gravel` | legacy steppe policy와 직접 대응한다. |
| `DryShrubland` | `DryShrublandRocky` | `coarse_dirt` | `dirt` | `stone` | `mud` | `exposed_rock` | `gravel` | shrubland 전용 legacy policy가 없으므로 rocky dryland family로 둔다. |
| `MediterraneanShrubland` | `MediterraneanShrubland` | `dry_grass` | `coarse_dirt` | `stone` | `grass` | `exposed_rock` | `gravel` | 이후 `leaf_litter` pocket을 cover override로 추가할 수 있다. |
| `PolarIce` | `Icefield` | `ice` | `snow` | `stone` | `ice` | `rock` | `moraine` | glacier/icefield와 alpine rock을 분리하기 위한 family다. |
| `PolarBarrens` | `TundraExposure` | `thin_soil` | `dirt` | `stone` | `moss` | `gravel` | `gravel` | legacy tundra exposure와 직접 대응한다. |
| `Tundra` | `TundraExposure` | `moss` | `dirt` | `stone` | `moss` | `gravel` | `gravel` | `snow`는 기본 top이 아니라 seasonal/frozen override가 우선한다. |
| `SubalpineWoodland` | `SubalpineWoodland` | `moss` | `thin_soil` | `stone` | `moss` | `gravel` | `gravel` | woodland/alpine edge transition family다. |
| `AlpineMeadow` | `AlpineMeadow` | `alpine_soil` | `moraine` | `stone` | `gravel` | `rock` | `gravel` | legacy `AlpineExposed`보다 meadow top을 우선한다. |
| `BorealForest` | `BorealForest` | `podzol` | `dirt` | `stone` | `moss` | `gravel` | `gravel` | block pool notes의 boreal/taiga 후보를 graph-first family로 승격한다. |
| `TropicalRainforest` | `TropicalLowland` | `jungle_grass` | `humus` | `dirt` | `mud` | `laterite` | `silt` | legacy tropical lowland policy와 직접 대응한다. |
| `MonsoonForest` | `MonsoonForest` | `jungle_grass` | `humus` | `dirt` | `mud` | `laterite` | `silt` | tropical lowland 기반이지만 seasonal wet/dry family로 분리한다. |
| `TropicalDryForest` | `TropicalDryForest` | `jungle_grass` | `humus` | `stone` | `mud` | `laterite` | `gravel` | tropical hills의 drier/exposed variant다. |
| `Savanna` | `SavannaGrassland` | `dry_grass` | `dirt` | `stone` | `grass` | `exposed_rock` | `gravel` | legacy savanna policy와 직접 대응한다. |
| `TemperateRainforest` | `TemperateRainforest` | `grass` | `humus` | `stone` | `mud` | `exposed_rock` | `silt` | temperate + organic/wet forest family가 새로 필요하다. |
| `TemperateMixedForest` | `TemperateMixedForest` | `grass` | `dirt` | `stone` | `mud` | `exposed_rock` | `gravel` | 이후 `leaf_litter` cover override를 추가할 수 있다. |
| `TemperateBroadleafForest` | `TemperateBroadleafForest` | `grass` | `humus` | `stone` | `mud` | `exposed_rock` | `gravel` | grassland보다 richer forest soil을 우선한다. |
| `TemperateGrassland` | `TemperateGrassland` | `grass` | `dirt` | `stone` | `mud` | `exposed_rock` | `gravel` | legacy temperate grassland policy와 직접 대응한다. |

초기 구현에서 새 family가 필요한 biome은 `DeepOcean`, `Mangrove`, `EstuarineCoast`, `LagoonCoast`,
`Lake`, `Swamp`, `FloodedForest`, `DryShrubland`, `MediterraneanShrubland`, `PolarIce`,
`SubalpineWoodland`, `AlpineMeadow`, `BorealForest`, `MonsoonForest`, `TropicalDryForest`,
`TemperateRainforest`, `TemperateMixedForest`, `TemperateBroadleafForest`다. 이 family들은 legacy policy를
대체하기보다 graph-first biome vocabulary에 맞춰 더 작은 launch palette를 붙이는 용도다.

---

## 첫 구현 범위

첫 구현은 final material system이 아니라 `grass` 단일 stub을 제거하는 launch slice다.

1. `src/world/generation/surface_plan/mod.rs`를 만들고, `SurfaceColumnInput`,
   `SurfaceColumnPlan`, `SurfacePlanArea`, `SurfacePlanConfig`, `SurfaceHydrologyRole`,
   `BiomeSurfacePolicy`를 정의한다.
2. `resolve_surface_column(input, config) -> SurfaceColumnPlan`을 단일 column 순수 함수로 구현한다.
3. `generate_surface_plan_area(heightfield, macro_field, config)`는 row-major order를 유지하며 deterministic
   parallel conversion을 허용한다. `MacroFieldTile`이 들어오면 heightfield column과 macro sample의
   world-space position이 같은지 먼저 검증한다. `SurfaceColumnPlan.world_x/world_z`는 sample position을
   포함하는 voxel block coordinate(`floor`)로 변환한다.
4. MVP policy는 아래 범위만 포함한다.
   - biome 기본 palette
   - ocean/lake/river/wetland/coast/dry-basin override
   - active ocean/lake/river/wetland/coast/dry-basin sediment override
   - owner-site 단위 non-water top material 정규화
5. `voxel`은 surface plan을 소비하도록 확장한다.
   - terrain top 1 block은 `top_block`
   - 그 아래 `soil_depth_blocks`는 `subsurface_block`
   - 더 아래는 `base_block`
   - `terrain_top_y < y <= water_y`는 water/ice block
6. vegetation placement는 계속 stage 15 이후 작업으로 남긴다.

첫 구현에서 preview 이미지는 새로 만들지 않아도 된다. 대신 unit test와 compile 검증은 반드시 수행한다.

---

## 추후 구현 계획

1. `HeightfieldColumn` 또는 pixelized downstream column에 biome/context를 직접 보존해
   `MacroFieldTile` compatibility input을 제거한다.
2. 3x3 neighbor slope/exposure, concavity, shoreline relative height를 `SurfacePlanArea` pass에서 계산하고,
   별도 rocky/cliff material override를 다시 도입할지 검토한다.
3. material transition을 단순 noise에서 boundary-aware stepping pass로 확장한다.
4. runtime `SurfaceCondition`과 계절/날씨 상태를 연결해 wet, snow-covered, frozen, thawing cover override를
   추가한다.
5. meso feature material hint와 vegetation placement를 연결한다.
6. policy table을 external data 또는 compact generated table로 분리할지 검토한다. 단, source of truth는
   계속 `world::generation::surface_plan` 계약이어야 한다.

---

## 불변식

1. biome owner와 visible material boundary는 분리될 수 있어야 한다.
2. ocean, lake, river, wetland, coast는 같은 water mask로 뭉개지면 안 된다.
3. material transition은 chunk 경계와 graph region 경계에 독립적이어야 한다.
4. surface plan은 renderer 리소스를 소유하지 않는다.
5. surface plan은 `ChunkData`를 직접 수정하지 않고 voxel fill이 소비할 계획을 만든다.
6. surface plan은 biome을 새로 resolve하지 않고 stage 8 final cell context와 macro_field cache가
   보존한 biome influence를 소비한다.
7. surface plan은 heightfield surface/water height를 바꾸지 않는다.
8. surface plan은 selected river, lake, ocean, wetland topology를 다시 선택하지 않는다.
9. deterministic variation은 chunk coordinate나 병렬 scheduling에 의존하면 안 된다.
10. block keys는 registry에 존재하는 launch block asset만 참조해야 한다.

---

## Plan 예시

surface plan은 block을 바로 쓰지 않고 column별 결정을 모은다.

```text
SurfaceColumnPlan {
  surface_y: 64,
  top_material: Grass,
  subsurface_material: Dirt,
  base_material: Stone,
  water_surface_y: None,
  hydrology_role: None,
  vegetation_allowed: true,
}
```

river floodplain column은 같은 위치라도 아래처럼 달라질 수 있다.

```text
SurfaceColumnPlan {
  surface_y: 61,
  top_material: Mud,
  subsurface_material: Silt,
  base_material: Stone,
  water_surface_y: Some(63),
  hydrology_role: River,
  vegetation_allowed: false,
}
```

이 plan들은 `ChunkData`를 수정하지 않는다. 마지막 voxel fill 단계가 water, material, vegetation
priority를 함께 보고 실제 block을 배치한다.

---

## 현재 구현 상태

- graph-first surface/material resolve launch slice가 `SurfaceColumnPlan`을 생성한다.
- biome 기본 palette, ocean/lake/river/wetland/coast/dry-basin override를 적용한 뒤,
  `MacroFieldSample.nearest_site` 기준으로 non-water noisy-owner top material을 단일화하고, 마지막에
  adjacent owner/material edge 주변에 material-only boundary wiggle을 적용한다.
- `MacroFieldTile` compatibility input이 연결된 area path는 heightfield/macro-field row-major length,
  dimensions, sample positions가 일치하지 않으면 panic으로 중단한다. 이 검증은 preview나 audit이 서로
  다른 좌표 footprint를 같은 column처럼 비교하는 회귀를 막기 위한 launch-scope contract다.
- `coast_mask`는 broad distance diagnostic으로 보존하지만, 그 값만으로 land biome을 coast/sand material로
  승격하지 않는다.
- rocky/exposure material override는 현재 기본 resolve에서 비활성이다. `terrain_ruggedness`,
  `ridge_influence`, `RockyCoast` biome, river bank roughness hint는 active water/sediment rule을 제외하고
  land/coast top material을 `gravel`/`wet_gravel`/`rock`/`exposed_rock`으로 승격하지 않는다.
- `SurfacePlanConfig::default()`의 `boundary_mix_radius_blocks`는 현재 `2`이고
  `boundary_mix_strength_percent`는 `36`이다. 다만 기본 surface plan preview/generation path처럼
  `MacroFieldTile` metadata가 연결된 경우, owner-normalization pass가 `MacroFieldSample.nearest_site`별
  non-water top material을 단일화한 뒤 final boundary wiggle pass가 adjacent owner/material edge에서만
  visual material을 작게 흔든다. 제외 대상은 hydrology role 자체가 아니라 `water_y`가 있는 실제 water
  column이다.
- seed `42`, center chunk `(-70, -32)`, radius `8` 기본 preview footprint의 contract data audit은
  noisy-owner base resolve와 final local-mix resolve를 둘 다 검사한다. `unsupported_local_mix_count = 0`이어야
  하며, final top material이 base material과 다를 경우 반드시 bounded orthogonal local mix 후보가 있어야
  한다. 또한 서로 다른 owner 사이에서 인접한 non-water final top material이 달라지는 edge는 해당 두
  noisy-owner site를 잇는 `NoisyBoundaryCurve`의 block-edge support 안에 있어야 한다. 같은 owner 내부의
  final top material conflict는 final boundary wiggle의 2-block visual feather로 허용한다. 이 audit은
  ignored unit test로 보존하며, PNG를 만들지 않고 macro field, Perlin-enabled heightfield, surface plan
  column data를 직접 비교한다.
- seed `42`, chunk `(-73, -40)`의 focused boundary audit은 1-block guard를 둔 column data를 직접 비교해
  noisy owner site, biome, base top block, final top block, local mix source neighbor를 출력한다. 이 audit
  역시 preview 이미지 없이 adjacent owner pair의 mutual material swap이 없는지,
  서로 다른 owner 사이의 material transition edge가 noisy boundary curve 자체의 block-edge support로
  지지되는지 검사한다.
