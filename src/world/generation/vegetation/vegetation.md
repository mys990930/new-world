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
- feature id와 world-space coordinate 기반 deterministic randomness 유지
- chunk 경계에 걸친 vegetation blueprint가 같은 결과로 재현되도록 anchor/footprint 계약 제공

---

## 비책임

- biome resolve
- surface material 선택
- terrain height 계산
- water fill
- `ChunkData` 직접 수정

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

---

## 불변식

1. vegetation placement는 chunk-local random choice에 의존하면 안 된다.
2. 같은 feature id와 world-space anchor는 같은 blueprint를 만들어야 한다.
3. vegetation은 water, cliff, active river channel, bare coast 같은 금지 mask를 존중해야 한다.
4. vegetation stage는 `ChunkData`를 직접 수정하지 않는다.
5. chunk 경계를 넘는 tree/feature는 어느 chunk를 먼저 생성해도 같은 block 결과를 내야 한다.
