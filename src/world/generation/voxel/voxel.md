# voxel

## 역할

`voxel`은 graph-first column plan을 `ChunkData`로 채우는 계약을 소유한다.

chunk는 저장과 출력 window일 뿐이다. 같은 world-space column은 어떤 chunk 생성 순서와 vertical
stack 생성 방식에서도 같은 voxel 결과를 내야 한다.

`voxel`은 heightfield나 surface policy 자체를 재해석하지 않는다. 대신 upstream
`heightfield`/`surface_plan` stage가 만든 column output을 읽어 surface, subsurface, water block
priority를 결정한다.

현재 graph-first created-world path는 구현된 `surface_plan`을 연결해 biome/material/water/coast
정책을 voxel fill에 반영한다. vegetation placement는 아직 생성하지 않는다.

---

## 책임

- column synthesis result를 chunk-local block fill로 변환
- heightfield column output을 읽어 world-space y 범위를 block stack으로 변환
- surface, subsurface, water, cave/void 후보를 `ChunkData`에 표현하는 계약
- heightfield, water, surface, vegetation plan의 priority resolve
- vertical chunk stack에서 같은 column plan을 재사용하는 규칙 정의
- registry block id와 material policy를 연결
- chunk boundary determinism과 stack determinism 검증 surface 제공
- `PixelizedChunkArea`와 `SurfacePlanArea`를 `GraphFirstVoxelPlan`으로 옮기고, 같은 x/z column plan을
  여러 vertical chunk stack이 공유하게 만든다.

---

## 비책임

- macro graph 생성
- hydrology solve
- final heightfield 계산
- Perlin noise sampling 또는 height 보정
- renderer mesh 생성
- save/load byte codec
- biome별 surface/material policy
- vegetation blueprint 또는 placement resolve

---

## 입력 계약

voxel fill은 앞 단계들의 산출물을 world-space column 단위로 읽는다. 이 단계는 Perlin을 다시
샘플하거나 surface height를 재계산하지 않는다. 새 graph-first path의 현재 구현 입력은
`PixelizedColumn`이며, 이 column은 `surface_y`, optional `water_y`, terrain kind hint, source macro
mask를 보존한다.

```text
HeightfieldColumn {
  world_xz: (120, -32),
  surface_y: 61,
  stone_floor_y: -256,
  soil_depth_blocks: 3,
  water_surface_y: Some(63),
  cave_or_void_intervals: [],
}

SurfaceColumnPlan {
  top_material: Mud,
  subsurface_material: Silt,
  base_material: Stone,
  hydrology_role: River,
  vegetation_allowed: false,
}

VegetationPlacements {
  placements_touching_column: [],
}
```

graph-first voxel plan은 `PixelizedColumn`의 위치/footprint handoff와 `SurfaceColumnPlan`의 최종
height/material handoff를 합쳐 아래 column plan으로 축약한다. surface-aware path에서는
`SurfaceColumnPlan.surface_y` / `water_y`가 authoritative 하다. `PixelizedColumn.surface_y` /
`water_y`는 compatibility hint이며, heightfield tile이 river descent 같은 area-level post-process를
적용한 뒤에는 surface plan과 달라질 수 있다.

```text
PixelizedColumn {
  world_x, world_z,
  chunk_x, chunk_z, local_x, local_z,
  surface_y,
  water_y,
}

GraphFirstVoxelColumnPlan {
  terrain_top_y,
  water_top_y,
  top_block_key,
  subsurface_block_key,
  base_block_key,
  underwater_top_block_key,
  water_block_key,
  soil_depth_blocks,
}
```

`water_y >= surface_y`인 standing-water column은 top visible surface가 물이 되도록
`terrain_top_y = min(surface_y, water_y - 1)`로 낮춘다. 그 결과 `terrain_top_y`에는
`underwater_top_block`, 그 아래 soil depth 범위에는 `subsurface_block`, 더 아래에는 `base_block`,
`terrain_top_y + 1 ..= water_top_y`에는 water, 그 위에는 air가 놓인다. 물이 없는 column의
`terrain_top_y`에는 `top_block`을 둔다.

---

## 공개 API

현재 구현된 graph-first launch API는 아래와 같다.

```rust
GraphFirstVoxelBuildConfig::new(seed, generator_version)
build_graph_first_voxel_plan(meta, min_chunk_x, max_chunk_x, min_chunk_z, max_chunk_z, config)
build_graph_first_voxel_plan_from_pixelized_area(area, fill_config)
build_graph_first_voxel_plan_from_pixelized_area_and_surface(area, surface, fill_config)
voxelize_graph_first_chunk(coord, plan, registry)
```

`build_graph_first_voxel_plan(...)`은 bounded x/z chunk range에 대해 graph, macro map, hydrology,
boundary, macro field, Perlin-enabled heightfield, surface plan, pixelize를 한 번 준비한 뒤
`GraphFirstVoxelPlan`을 만든다. 이 plan은 y chunk를 포함하지 않는다. 같은 plan을
`voxelize_graph_first_chunk(...)`에 여러 `ChunkCoord(x, y, z)`로 넘기면 vertical stack이 같은 x/z
column을 재사용한다.

이 입력을 합치면 voxel fill은 해당 column에서 `y <= 57`은 stone, `58..=60`은 silt, `61`은 mud,
`62..=63`은 water, 그 위는 air로 채운다. 만약 vegetation placement가 해당 column을 덮으면,
terrain/water를 먼저 정한 뒤 priority 규칙에 따라 trunk/leaves/grass 같은 block을 얹는다.

---

## Voxel Fill 원칙

- chunk-local random choice를 금지한다.
- 필요한 randomness는 `seed + generator_version + graph owner id + feature id + world-space coordinate`에서 결정한다.
- output chunk coordinate는 채울 범위를 고를 뿐 terrain identity를 만들지 않는다.
- vertical chunks는 같은 x/z column plan을 공유해야 한다.
- water surface, lake flattening, river corridor는 heightfield와 surface plan의 결과를 따른다.
- surface, water, vegetation stage는 `ChunkData`를 직접 수정하지 않고 plan만 만든다.
- voxel fill은 모든 plan을 모아 priority 순서대로 block을 배치한다.
- 현재 priority는 단순하다.
  - `world_y == terrain_top_y` and water above terrain: `underwater_top_block`
  - `world_y == terrain_top_y` and no water above terrain: `top_block`
  - `terrain_top_y - soil_depth_blocks .. terrain_top_y - 1`: `subsurface_block`
  - lower terrain: `base_block`
  - `terrain_top_y < world_y <= water_top_y`: `water_block`
  - 그 위: `air`
  - vegetation은 아직 없음

---

## Chunk Fill 예시

`CHUNK_EDGE = 32`이고, 위 column이 `ChunkCoord(3, 1, -1)`에 들어 있다고 하자. 이 chunk의
world-space y 범위는 `32..=63`이다. voxel fill은 local y를 world y로 바꾼 뒤 앞 단계의 plan을
읽는다.

```text
world_y = chunk_y * CHUNK_EDGE + local_y
```

```text
local_y  world_y  block
0..=25   32..=57  Stone
26..=28  58..=60  Silt
29       61       Mud
30..=31  62..=63  Water
```

같은 column의 `ChunkCoord(3, 2, -1)`는 world-space y 범위가 `64..=95`이므로 기본적으로 air다.
다만 vegetation placement가 이 y 범위를 덮으면 leaves/trunk block이 들어갈 수 있다. 반대로
`ChunkCoord(3, 0, -1)`는 `0..=31`이므로 이 column에서는 전부 Stone으로 채워진다.

숲 column이라면 heightfield/surface/water plan은 그대로 두고 vegetation plan이 별도로 들어온다.

```text
VegetationPlacement {
  anchor: (120, 65, -32),
  blueprint: OakSmall,
  voxels: trunk/leaves offsets...
}
```

마지막 voxel fill은 같은 `ChunkData`에 먼저 terrain/water를 채운 뒤 vegetation voxel을 priority에
따라 얹는다. 이 구조를 쓰면 material, water, vegetation 단계가 서로 다른 순서로 `ChunkData`를
직접 덮어쓰는 문제를 피할 수 있다.

---

## 불변식

1. 같은 `(seed, generator_version, world_x, world_z)`는 같은 column plan을 만든다.
2. 같은 `(seed, generator_version, chunk_coord)`는 같은 `ChunkData`를 만든다.
3. chunk별 생성과 area/stack batch 생성 결과가 일치해야 한다.
4. 인접 chunk 경계 column이 일치해야 한다.
5. voxel fill은 graph region 사각 경계를 드러내면 안 된다.
6. plan priority는 deterministic해야 하며 stage 실행 순서가 block 결과를 바꾸면 안 된다.
7. voxel fill은 heightfield를 재계산하지 않고 heightfield column output을 소비해야 한다.

---

## 현재 구현 상태

- `src/world/generation/voxel/mod.rs`는 graph-first launch voxel fill을 구현한다.
- `GraphFirstVoxelPlan`은 `PixelizedChunkArea`와 `SurfacePlanArea`에서 변환되며, pixelized area는
  world/chunk/local footprint를 제공하고 surface plan은 최종 height/water/material policy를 제공한다.
  vegetation은 아직 stub이다.
- terrain은 surface plan의 `top_block`, `subsurface_block`, `base_block`, `underwater_top_block`을
  사용하고, water hint가 있는 column은 registry의 `water` block을 사용한다.
- `world_create`는 이 graph-first plan을 사용해 bounded created-world dump를 저장할 수 있다.
- 이 구현은 surface/material policy를 반영하지만 vegetation/cave/void priority는 아직 들어오지 않은
  vertical slice다.
