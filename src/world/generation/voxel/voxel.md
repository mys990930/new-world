# voxel

## 역할

`voxel`은 graph-first column plan을 `ChunkData`로 채우는 계약을 소유한다.

chunk는 저장과 출력 window일 뿐이다. 같은 world-space column은 어떤 chunk 생성 순서와 vertical
stack 생성 방식에서도 같은 voxel 결과를 내야 한다.

`voxel`은 heightfield를 생성하지 않는다. 대신 heightfield stage가 만든 column output을 읽어
surface, water, vegetation plan과 함께 block priority를 결정한다.

---

## 책임

- column synthesis result를 chunk-local block fill로 변환
- heightfield column output을 읽어 world-space y 범위를 block stack으로 변환
- surface, subsurface, water, cave/void 후보를 `ChunkData`에 표현하는 계약
- heightfield, water, surface, vegetation plan의 priority resolve
- vertical chunk stack에서 같은 column plan을 재사용하는 규칙 정의
- registry block id와 material policy를 연결
- chunk boundary determinism과 stack determinism 검증 surface 제공

---

## 비책임

- macro graph 생성
- hydrology solve
- final heightfield 계산
- Perlin noise sampling 또는 height 보정
- renderer mesh 생성
- save/load byte codec

---

## 입력 계약

voxel fill은 앞 단계들의 산출물을 world-space column 단위로 읽는다. 이 단계는 Perlin을 다시
샘플하거나 surface height를 재계산하지 않는다.

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
