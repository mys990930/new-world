# voxel

## 역할

`voxel`은 graph-first column plan을 `ChunkData`로 채우는 계약을 소유한다.

chunk는 저장과 출력 window일 뿐이다. 같은 world-space column은 어떤 chunk 생성 순서와 vertical
stack 생성 방식에서도 같은 voxel 결과를 내야 한다.

---

## 책임

- column synthesis result를 chunk-local block fill로 변환
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
- renderer mesh 생성
- save/load byte codec

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

## Plan 기반 fill 예시

입력 plan:

```text
ColumnPlan {
  world_xz: (120, -32),
  surface_y: 61,
  stone_floor_y: -256,
  water_surface_y: Some(63),
  top_material: Mud,
  subsurface_material: Silt,
  base_material: Stone,
  vegetation: [],
}
```

voxel fill은 chunk의 y 범위와 이 column plan을 비교해 block을 결정한다.

```text
if y > surface_y && y <= water_surface_y:
    Water
else if y == surface_y:
    top_material
else if y >= surface_y - soil_depth:
    subsurface_material
else if y >= stone_floor_y:
    base_material
else:
    Air 또는 world floor policy
```

숲 column이라면 surface/water plan은 그대로 두고 vegetation plan이 별도로 들어온다.

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
