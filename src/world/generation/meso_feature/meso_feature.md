# meso_feature

## 역할

`meso_feature`는 macro guide보다 작고 Perlin micro relief보다 큰 국소 지형 feature의 planning
계약을 소유한다.

이 단계는 crater, ravine, dune field, hill cluster, terrace, closed basin, escarpment pocket 같은
지역적 지형 객체를 deterministic feature plan으로 만든다. feature plan은 `ChunkData`를 직접
수정하지 않고, heightfield와 surface plan이 읽을 deformation/mask/material hint를 제공한다.

---

## 책임

- graph, macro map, hydrology, boundary, field context를 읽어 meso feature 후보를 고른다.
- feature id, owner graph id, world-space anchor, footprint, priority, seed를 정의한다.
- heightfield가 읽을 height delta, flatten, carve, raise, mask, keep-out 계약을 제공한다.
- surface plan이 읽을 material hint와 exposure/sediment/soil support를 제공한다.
- feature footprint가 chunk 또는 graph region 경계를 넘어도 같은 결과가 재현되도록 한다.
- 각 meso feature category가 topdown preview에 독립적으로 나타나게 한다.

---

## 비책임

- continent/ocean ownership 결정
- primary ridge/fault/coast guide 결정
- primary hydrology routing
- Perlin noise sampling
- block placement
- live world storage mutation

---

## Pipeline 위치

meso feature planning은 아래 정보가 준비된 뒤 실행한다.

- padded Voronoi graph
- base graph field
- continent/ocean macro elevation
- ridge/fault/mountain/coast edge guide
- hydrology solve와 selected river chain
- noisy boundary realization
- Voronoi-derived macro field/noise map

그리고 아래 단계보다 먼저 실행한다.

- Perlin micro relief
- final heightfield and water surface
- biome/material/surface plan
- vegetation plan
- voxel fill

즉, meso feature는 macro 구조와 hydrology를 읽고, Perlin과 final heightfield가 그 feature를
존중하도록 만드는 중간 지형 객체다.

---

## Feature 예시

launch 후보:

- `hill_cluster`: 평야나 완만한 고원에 생기는 여러 봉우리와 shoulder
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

대부분의 meso feature는 hydrology 이후에 계획된다. 따라서 selected river를 끊거나 ocean/lake
의미를 뒤집으면 안 된다.

정책:

- selected river corridor, lake surface, coast outlet은 기본 keep-out 또는 protected mask다.
- river와 상호작용하는 feature는 floodplain, terrace, side ravine처럼 hydrology role을 읽어야 한다.
- feature가 primary hydrology routing 자체를 바꿔야 한다면 meso feature가 아니라 macro/hydrology guide로 승격한다.
- graph-stage lake/sink/outlet carve를 새로 만들 정도의 feature는 hydrology solve 이전 guide로 선언해야 한다.
- Perlin 이후 micro depression처럼 작은 국소 함몰은 meso hydrology가 아니라 heightfield cleanup 정책으로 처리한다.

---

## Plan 형태

```text
MesoFeaturePlan {
  id: FeatureId(...),
  kind: Ravine,
  owner: VoronoiSiteId(...),
  anchor_xz: (1200.0, -320.0),
  footprint_radius_blocks: 96.0,
  priority: 40,
  seed: ...,
  height_delta_rule: Carve { depth_blocks: 8.0, shoulder_blocks: 24.0 },
  material_hint: ExposedRockAndGravel,
  protected_masks: [SelectedRiverChannel, LakeSurface, CoastOutlet],
}
```

heightfield는 이 plan을 읽어 해당 footprint 안의 column height를 낮추거나 올린다. surface plan은
같은 plan의 material hint를 읽어 rock, gravel, sand, mud 같은 전환을 결정할 수 있다.

---

## Preview

초기 preview는 최소한 아래를 보여야 한다.

- feature owner와 anchor
- footprint / falloff
- height delta preview
- protected mask와 충돌한 feature rejection
- feature kind별 color map
- final heightfield에 반영된 feature contribution

---

## 불변식

1. meso feature는 chunk-local random choice에 의존하면 안 된다.
2. 같은 feature id, owner graph id, seed는 같은 feature footprint와 deformation을 만든다.
3. meso feature는 macro continent/ocean ownership을 뒤집지 않는다.
4. meso feature는 selected river, lake, coast outlet을 명시적 정책 없이 끊거나 막으면 안 된다.
5. hydrology routing을 바꾸는 feature는 meso 단계가 아니라 macro/hydrology guide로 승격해야 한다.
6. feature footprint가 chunk나 graph region 경계를 넘어도 overlap 영역 결과가 같아야 한다.
7. meso feature output은 `ChunkData`가 아니라 heightfield/surface plan이 소비할 plan이어야 한다.
