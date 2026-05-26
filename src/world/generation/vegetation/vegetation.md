# vegetation

## 역할

`vegetation`은 biome, hydration, surface, hydrology, slope를 읽어 vegetation과 surface feature
placement plan을 만드는 계약을 소유한다.

이 단계는 나무나 풀 block을 바로 `ChunkData`에 쓰지 않는다. deterministic placement 후보와
blueprint reference를 만들고, 마지막 voxel fill 단계가 terrain/water/surface priority와 함께 실제
block을 배치한다.

---

## 책임

- biome별 vegetation density와 species 후보 선택
- hydration, temperature, elevation, slope, river/lake/wetland/coast role에 따른 placement mask 계산
- tree, shrub, grass, reed, ground cover 같은 vegetation placement plan 생성
- small rock, fallen branch, stump 같은 surface feature placement를 world-owned prop reference로 생성
- beach, dry riverbank, sandy sediment 같은 surface-feature target mask 계산
- feature id와 world-space coordinate 기반 deterministic randomness 유지
- chunk 경계에 걸친 vegetation blueprint가 같은 결과로 재현되도록 anchor/footprint 계약 제공

---

## 비책임

- biome resolve
- surface material 선택
- terrain height 계산
- water fill
- `ChunkData` 직접 수정
- prop asset parsing or microvoxel mesh baking
- renderer object creation

---

## Placement 입력

vegetation plan은 아래 입력을 함께 본다.

- final biome influence
- surface material과 soil/sediment class
- hydration, temperature, elevation
- slope와 exposure
- river/lake/wetland/coast role
- floodplain, beach, cliff, ridge mask
- deterministic feature seed
- available prop definition ids for surface features such as small stones

---

## Plan 예시

```text
VegetationPlacement {
  id: FeatureId(...),
  anchor: WorldBlockCoord(120, 65, -32),
  kind: OakSmall,
  footprint_radius_blocks: 3,
  blueprint_seed: ...,
}
```

이 plan은 anchor가 현재 chunk 밖에 있어도 이웃 chunk voxel fill에서 같은 tree footprint를 볼 수
있어야 한다. 따라서 voxel fill은 자기 chunk 내부 anchor만 보지 않고, chunk bounds와 겹치는
nearby vegetation placement를 함께 조회해야 한다.

Rock and other small surface features follow the same deterministic placement rule, but they should reference a
world-owned prop definition rather than describe renderer geometry directly:

```text
SurfaceFeaturePlacement {
  id: FeatureId(...),
  anchor: WorldBlockCoord(124, 66, -29),
  kind: PropRef("rock_stacked_2x2x1_plus_1"),
  footprint_radius_blocks: 1,
  blueprint_seed: ...,
}
```

The prop definition owns the one-block semantic meaning and any microvoxel visual shape. Vegetation placement only
selects where the feature may exist.

---

## 불변식

1. vegetation placement는 chunk-local random choice에 의존하면 안 된다.
2. 같은 feature id와 world-space anchor는 같은 blueprint를 만들어야 한다.
3. vegetation은 water, cliff, active river channel, bare coast 같은 금지 mask를 존중해야 한다.
4. vegetation stage는 `ChunkData`를 직접 수정하지 않는다.
5. chunk 경계를 넘는 tree/feature는 어느 chunk를 먼저 생성해도 같은 block 결과를 내야 한다.
6. rock and small surface feature placement must emit prop placement references, not renderer meshes or texture
   layers.
7. prop placement ids must be deterministic from feature id, prop definition id, and world-space anchor.
8. small rock placement must stay sparse and targeted; sandy/coastal or dry riverside sediment surfaces are valid
   early targets, but ordinary grassland/forest floor columns are not.

---

## 현재 구현 상태

- graph-first 저장 vertical slice에서는 tree/grass vegetation placement를 생성하지 않는다.
- `generate_rock_feature_placements(area, config)`는 `SurfacePlanArea`의 dry beach/coast 또는 dry riverside
  sediment column을 읽어 deterministic `SurfaceFeaturePlacement` prop references를 만든다. active water
  column과 ordinary grassland/forest floor는 rock 후보에서 제외한다.
- `world_create`가 사용하는 launch voxel fill은 vegetation plan을 빈 plan으로 취급한다.
- 나무, 풀, 수생 식생, chunk 경계 feature anchor 정책은 후속 구현 범위이며 현재 `ChunkData`에는
  terrain/water만 기록된다.
- small rock props are the first surface-feature vertical slice. Placement emits prop keys such as
  `rock_pebble_1x1x1`, while `legacy/prop.md` owns the microvoxel prop asset and mesh-bake contract.
