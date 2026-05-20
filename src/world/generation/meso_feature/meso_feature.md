# meso_feature

## 역할

`meso_feature`는 macro guide보다 작고 Perlin micro relief보다 큰 국소 지형 feature의 planning
계약을 소유한다.

이 단계는 crater, ravine, dune field, hill cluster, terrace, closed basin, escarpment pocket 같은
지역적 지형 객체를 deterministic feature plan으로 만든다. feature plan은 `ChunkData`를 직접
수정하지 않고, stage 11 `macro_field`가 읽어 raster/sample channel과 `combined_macro_height`에 굽는
중간 입력이다. `heightfield`와 `surface_plan`은 meso geometry를 다시 해석하지 않고, macro_field와
pixelize가 보존한 meso-baked column channel과 material/surface hint를 소비한다.

---

## 책임

- graph, macro map, hydrology, river plan, final cell context, boundary context를 읽어 meso feature 후보를 고른다.
- feature id, owner graph id, world-space anchor, footprint, falloff, priority, seed를 정의한다.
- macro_field가 읽을 height contribution rule을 제공한다.
  - raise: 작은 언덕, 봉우리, shoulder, upland knob처럼 높이를 더하는 feature
  - carve: ravine, dry cut, shallow hollow처럼 지형을 낮추는 feature
  - flatten: terrace, bench, playa edge처럼 제한된 footprint를 완만하게 정리하는 feature
  - roughness: macro shape를 뒤집지 않는 중간 규모 표면 변주
- macro_field가 읽을 mask와 keep-out/protected area 계약을 제공한다.
- surface_plan이 읽을 material hint와 exposure/sediment/soil support를 제공한다.
- feature footprint가 chunk 또는 graph region 경계를 넘어도 같은 결과가 재현되도록 한다.
- 각 meso feature category가 topdown preview에 독립적으로 나타나게 한다.

---

## 비책임

- continent/ocean ownership 결정
- primary ridge/fault/coast guide 결정
- primary hydrology routing
- selected river chain, lake/sink/outlet resolution 변경
- macro field tile rasterization 자체
- Perlin noise sampling
- block placement
- live world storage mutation

---

## Pipeline 위치

meso feature planning은 아래 정보가 준비된 뒤 실행한다.

- padded Voronoi graph
- base graph field
- continent/ocean/island ownership과 signed macro elevation
- ridge/fault/coast edge guide, plus mountainness/rugged context fields
- hydrology solve와 selected river chain
- river plan reach morphology
- final cell context, climate/hydration/biome influence
- noisy boundary realization

그리고 아래 단계보다 먼저 실행한다.

- macro field rasterization
- chunk pixelize
- final heightfield and water surface realization
- Perlin micro relief
- biome/material/surface plan
- vegetation plan
- voxel fill

즉, meso feature는 macro 구조와 hydrology가 확정된 뒤 생성되지만, visible height cache가 만들어지기
전에는 반드시 계획되어야 한다. `macro_field`는 이 plan을 읽어 meso contribution을 sample grid에
bake하고, 이후 `pixelize`와 `heightfield`는 그 결과를 재해석 없이 소비한다.

---

## Macro Field Handoff

`MesoFeaturePlan`은 vector/object 형태의 feature table이다. macro_field는 tile cache miss에서 이
table을 읽어 각 sample에 deterministic meso channel을 만든다.

예상 channel:

- `meso_raise_strength`
- `meso_carve_strength`
- `meso_flatten_strength`
- `meso_roughness_strength`
- `meso_delta_blocks`
- `meso_material_hint`
- `meso_protected_mask`

`combined_macro_height`는 macro elevation, ocean/lake shape, river broad valley, ridge policy와 함께
meso contribution이 반영된 pre-Perlin height다. 이 값이 pixelize와 heightfield가 읽는 기본 terrain
height source다.

중요한 경계:

- meso feature는 macro ownership을 뒤집지 않는다.
- meso feature는 selected river, lake surface, ocean/coast outlet을 명시적 정책 없이 막지 않는다.
- hydrology routing을 바꿀 정도의 feature는 ordinary meso가 아니다. 그런 feature는 macro/hydrology
  guide로 승격해 stage 6 이전 입력으로 다뤄야 한다.
- macro_field가 meso plan을 bake한 뒤에는 downstream stage가 feature anchor/footprint를 다시 탐색하지 않는다.

---

## Feature 예시

launch 후보:

- `hill_cluster`: 평야나 완만한 고원에 생기는 여러 봉우리와 shoulder
- `upland_knob`: 고지대나 ridge shoulder 주변의 작은 봉우리
- `secondary_spur`: 큰 ridge/fault guide에서 옆으로 뻗는 짧은 능선
- `closed_basin`: graph-stage hydrology를 뒤집지 않는 얕은 국소 분지
- `ravine`: selected river와 별개인 건식 침식 지형 또는 hydrology-supported side cut
- `dune_field`: 건조하고 노출된 sediment/sand support 영역의 반복 능선
- `terrace`: coast, river, plateau edge 주변의 계단형 완만한 지형
- `crater`: sparse impact/depression feature
- `sinkhole_field`: 특정 lithology/material hint가 있는 지역의 작은 함몰 군집
- `escarpment_pocket`: macro fault/cliff guide 주변의 국소 절벽 강화

feature별 세부 구현이 커지면 `meso_feature/<feature>/<feature>.md` 문서와 leaf 구현으로 분리한다.

---

## Hydrology와의 관계

대부분의 meso feature는 hydrology 이후에 계획된다. 따라서 selected river를 끊거나 ocean/lake 의미를
뒤집으면 안 된다.

정책:

- selected river corridor, lake surface, coast outlet은 기본 keep-out 또는 protected mask다.
- river와 상호작용하는 feature는 floodplain, terrace, side ravine처럼 hydrology role과 river plan
  reach morphology를 읽어야 한다.
- river-adjacent meso는 강줄기를 새로 routing하지 않고, macro_field가 이미 가진 river core/shoulder
  guide와 합성 가능한 height/material hint만 제공한다.
- feature가 primary hydrology routing 자체를 바꿔야 한다면 meso feature가 아니라 macro/hydrology guide로 승격한다.
- graph-stage lake/sink/outlet carve를 새로 만들 정도의 feature는 hydrology solve 이전 guide로 선언해야 한다.
- Perlin 이후 micro depression처럼 작은 국소 함몰은 meso hydrology가 아니라 heightfield cleanup 정책으로 처리한다.

---

## Plan 형태

```text
MesoFeaturePlan {
  id: FeatureId(...),
  kind: HillCluster,
  owner: VoronoiSiteId(...),
  anchor_xz: (1200.0, -320.0),
  footprint_radius_blocks: 96.0,
  falloff: SmoothShoulder { inner_blocks: 24.0, outer_blocks: 96.0 },
  priority: 40,
  seed: ...,
  height_rule: Raise { height_blocks: 18.0, roughness_blocks: 3.0 },
  material_hint: ExposedSoilAndRock,
  protected_masks: [SelectedRiverChannel, LakeSurface, CoastOutlet],
}
```

macro_field는 이 plan을 읽어 해당 footprint 안의 sample height를 올리거나 낮추고, 필요한 경우
flatten/roughness/material hint를 sample channel로 보존한다. surface_plan은 pixelize/heightfield가
보존한 같은 hint를 읽어 rock, gravel, sand, mud 같은 전환을 결정할 수 있다.

---

## Preview

초기 preview는 최소한 아래를 보여야 한다.

- feature owner와 anchor
- footprint / falloff
- height contribution preview
- protected mask와 충돌한 feature rejection
- feature kind별 color map
- macro_field에 bake된 meso contribution channel
- final combined macro height에서 meso contribution이 반영된 결과

---

## 불변식

1. meso feature는 chunk-local random choice에 의존하면 안 된다.
2. 같은 feature id, owner graph id, seed는 같은 feature footprint와 deformation을 만든다.
3. meso feature는 macro continent/ocean ownership을 뒤집지 않는다.
4. meso feature는 selected river, lake, coast outlet을 명시적 정책 없이 끊거나 막으면 안 된다.
5. hydrology routing을 바꾸는 feature는 meso 단계가 아니라 macro/hydrology guide로 승격해야 한다.
6. feature footprint가 chunk나 graph region 경계를 넘어도 overlap 영역 결과가 같아야 한다.
7. meso feature output은 `ChunkData`가 아니라 macro_field가 소비할 deterministic plan이어야 한다.
8. macro_field 이후 stage는 meso feature geometry를 다시 탐색하거나 재소유하지 않는다.
