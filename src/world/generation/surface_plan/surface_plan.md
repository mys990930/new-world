# surface_plan

## 역할

`surface_plan`은 heightfield와 stage 7 final cell context, hydrology role을 읽어 biome/material/water/coast
surface policy를 resolve하는 plan 계약을 소유한다.

이 단계는 visible material boundary를 안정적으로 만들지만, hard polygon owner를 그대로 색칠하지
않는다.

---

## 책임

- 이미 resolve된 biome influence와 hydrology role에서 surface policy 선택
- ocean, coast, lake, river, wetland, floodplain의 material 의미 구분
- slope, exposure, elevation, hydration에 따른 beach/cliff/rock/soil/vegetation 전환
- column voxel fill이 읽을 surface column plan 정의
- seasonal/runtime surface state와 연결될 수 있는 world-side 계약 유지

---

## 비책임

- graph construction
- hydrology solve
- final height 계산
- block storage mutation
- renderer material upload

---

## Resolve 입력

surface resolve는 아래 입력을 함께 본다.

- stage 7 final cell context의 continuous temperature / hydration / ruggedness
- dominant site와 blended biome influence
- heightfield surface y와 slope
- ocean/lake/river/wetland/coast role
- river flow accumulation과 floodplain width
- ridge/fault/cliff guide
- meso feature material hint와 deformation mask
- local soil/sediment class
- runtime season/weather state가 허용하는 override

---

## Material Transition

visible material boundary는 hard owner 경계를 그대로 따라가면 안 된다.

권장 방식:

- continuous field gradient
- domain-warped boundary distance
- deterministic dithering
- cover override
- hydrology role 우선순위
- slope/exposure 기반 rocky override

---

## 불변식

1. biome owner와 visible material boundary는 분리될 수 있어야 한다.
2. ocean, lake, river, wetland, coast는 같은 water mask로 뭉개지면 안 된다.
3. material transition은 chunk 경계와 graph region 경계에 독립적이어야 한다.
4. surface plan은 renderer 리소스를 소유하지 않는다.
5. surface plan은 `ChunkData`를 직접 수정하지 않고 voxel fill이 소비할 계획을 만든다.
6. surface plan은 biome을 새로 resolve하지 않고 stage 7 final cell context와 macro_field cache가
   보존한 biome influence를 소비한다.

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

- graph-first 저장 vertical slice에서는 아직 실제 surface/material resolve를 구현하지 않았다.
- `world_create`가 사용하는 launch voxel fill은 이 단계를 stub으로 넘기며, 비물 지형은 임시로
  registry의 `grass` block 하나만 사용한다.
- ocean/lake/river/coast별 material policy는 이 문서의 계약으로 남아 있으며, 현재 stub이 최종 정책을
  대체하지 않는다.
