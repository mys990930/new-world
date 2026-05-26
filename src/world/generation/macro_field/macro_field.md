# macro_field

## 역할

`macro_field`는 graph-first generator의 11단계 raster/scalar cache다. 입력 stage가 만든
`VoronoiGraphPatch`, `GraphMacroMap`, final cell context, `BoundaryCache`, `MesoFeaturePlan`을
tile sample로 굽고, pixelize/heightfield/preview가 빠르게 읽을 수 있는 `MacroFieldTile`을 만든다.

현재 river realization은 의도적으로 reset 상태다. API는 `RiverPlan`을 계속 받지만
`macro_field`는 river valley, river bed, river core, estuary fan, river water hint를 만들지 않는다.
강 topology와 Q/morphology planning은 upstream `hydrology` / `river_plan`에 남아 있으며, 이 단계에서
terrain으로 실현하는 구현은 다음 설계에서 다시 작성한다.

---

## 책임

- tile bounds, resolution, world-space sample spacing 계약을 정의한다.
- noisy boundary side query로 owner/mask sample을 resolve한다.
- macro elevation, ocean/coast/lake/dry basin mask, ridge/coast influence, meso contribution,
  final biome context를 sample channel로 보존한다.
- `MesoFeaturePlan`의 raise/carve/flatten/roughness contribution을 `combined_macro_height`에 bake한다.
- `combined_macro_height`를 heightfield 이전의 macro/meso terrain source로 제공한다.
- contour preview/debug가 읽을 수 있는 block-space contour diagnostic을 제공한다.
- river 관련 public fields는 downstream 호환을 위해 남기되 모두 neutral 값으로 채운다.

---

## 비책임

- Voronoi graph 생성
- macro ownership/elevation source 결정
- hydrology routing 또는 selected river 결정
- river reach morphology, river valley/core/bed carve, estuary fan 생성
- Perlin/fBM micro relief 생성
- final water surface solve
- material/vegetation/voxel fill

---

## River Reset 계약

- `RiverPlan` 인자는 facade/API 호환용이다. 현재 `MacroFieldRasterContext`는 river source curve,
  river grid, estuary fan을 만들지 않는다.
- `rasterize_influence_fields`는 ridge/coast distance field만 만든다. river source curve/pixel stats는
  항상 `0`이다.
- `MacroFieldSample`의 river/estuary fields는 다음 neutral 값을 사용한다.
  - strengths, flow, depth, roughness, gravel, cutbank, longitudinal hints: `0.0`
  - `river_distance_blocks`: `f32::INFINITY`
- `combined_macro_height`는 river fields를 읽지 않는다. 현재 합성 source는 macro elevation,
  ocean bathymetry, lake/wetland lowering, ridge stub, meso contribution이다.
- preview가 selected river centerline overlay를 그리고 싶다면 upstream `RiverPlan`과
  `BoundaryCache`를 직접 읽는 진단 layer로만 처리해야 한다. 이 overlay는 `MacroFieldSample`의
  terrain source가 아니다.

---

## 공개 API

```rust
MacroFieldTileConfig::new(origin_x, origin_z, width, height, sample_spacing_blocks)

generate_macro_field_tile(
    &VoronoiGraphPatch,
    &GraphMacroMap,
    &RiverPlan,
    &BoundaryCache,
    MacroFieldTileConfig,
) -> MacroFieldTile
```

주요 output:

```rust
MacroFieldSample {
    position,
    nearest_site,
    surface_kind,
    biome_context,
    biome,
    macro_elevation,
    ocean_mask,
    coast_mask,
    lake_mask,
    dry_basin_mask,
    ridge_influence,
    river_core_strength,       // reset: 0
    river_shoulder_strength,   // reset: 0
    river_valley_strength,     // reset: 0
    river_distance_blocks,     // reset: infinity
    river_flow_hint,           // reset: 0
    river_longitudinal_blocks, // reset: 0
    river_core_depth_hint,     // reset: 0
    river_bank_roughness_hint, // reset: 0
    river_gravel_hint,         // reset: 0
    river_cutbank_hint,        // reset: 0
    estuary_water_strength,    // reset: 0
    estuary_water_depth_hint,  // reset: 0
    meso_*,
    combined_macro_height,
}
```

---

## 파일 구조

- `mod.rs`: public facade, tile generation orchestration, sample assembly.
- `types.rs`: public tile config, sample, tile, stats types and validation defaults.
- `context.rs`: raster context, owner lookup, biome lookup, boundary/junction/spatial index helpers.
- `geometry.rs`: generic polyline/site geometry helpers shared by boundary/ridge/coast code.
- `influence.rs`: ridge/coast tile-local influence raster pass and stats aggregation.
- `height.rs`: combined/ocean/lake height policy plus envelope/roughness helpers.
- `ocean.rs`: isolated ocean-fragment cleanup.
- `contour.rs`: contour data and Marching Squares diagnostic extraction.
- `test_support.rs`: `cfg(test)` fixtures for macro-field tests.

`river.rs` is intentionally absent in this reset state. Reintroducing river realization should add a
new documented owner boundary first, then implementation/tests.

---

## 불변식

1. `macro_field`는 upstream graph/macro/hydrology/river-plan/final-cell-context/boundary/meso-feature
   source of truth를 바꾸지 않는다.
2. tile generation is deterministic and safe to run in parallel.
3. `combined_macro_height` must be finite for every sample.
4. river/estuary sample channels remain neutral until a new river realization design is documented.
5. heightfield must not infer missing river terrain from neutral river fields.
