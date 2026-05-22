# pixelize

## 역할

`pixelize`는 graph-first generator의 12단계인 chunk pixelize 계약을 소유한다.

이 단계는 stage 11 `macro_field`가 만든 `MacroFieldTile`을 읽어, 청크 경계에 정렬된
`1 world block = 1 pixel = 1 voxel column` 해상도의 resolved column cache로 바꾼다. 즉
`macro_field`가 graph-derived source field cache라면, `pixelize`는 그 field를 chunk/window 단위
column layout으로 확정하는 첫 단계다.

`pixelize`는 graph topology, hydrology, noisy boundary, river plan, meso feature geometry를 다시 해석하지 않는다. 모든 macro
의미와 meso contribution은 입력 `MacroFieldSample`에 이미 들어 있어야 하며, 이 단계는 그 값을 column 좌표계와 integer
surface/water hint로 옮긴다.

---

## 책임

- chunk-aligned output footprint와 column ordering 계약 정의
- `MacroFieldTile` sample을 world/chunk/local 좌표가 있는 `PixelizedColumn`으로 변환
- `combined_macro_height`를 shared block-height domain으로 읽어 integer `surface_y`를 resolve
- ocean/lake/river hint에서 optional integer `water_y`를 만든다
- terrain kind hint, source macro masks, meso-baked source channel을 downstream stage가 잃지 않도록 보존
- `MacroFieldTile`의 `sample_spacing_blocks = 1.0` handoff를 runtime/cache 계약으로 고정
- deterministic parallel conversion을 허용하되 output order는 chunk/local/world index 기준으로 안정화

---

## 비책임

- Voronoi graph 생성 또는 nearest site 재탐색
- macro ownership, hydrology, river reach morphology, noisy boundary 판정
- meso feature 해석 또는 Perlin micro relief 생성
- biome/material/surface policy resolve
- final `ChunkData` voxel fill
- preview PNG encoding 또는 renderer/GPU 리소스 생성

---

## 공개 API

현재 public shape는 stage 12의 chunk-aligned column cache를 노출한다.

```rust
PixelizeConfig::default()
generate_pixelized_chunk_area(&MacroFieldTile, PixelizeConfig) -> PixelizedChunkArea
pixelized_column_from_macro_sample(&MacroFieldSample, PixelizeConfig) -> PixelizedColumn
```

주요 데이터:

```rust
PixelizeConfig {
    heightfield: HeightfieldConfig,
}

PixelizedChunkArea {
    origin_world_x,
    origin_world_z,
    width,
    height,
    min_chunk_x,
    max_chunk_x,
    min_chunk_z,
    max_chunk_z,
    columns,
    stats,
    config,
}

PixelizedColumn {
    world_x,
    world_z,
    chunk_x,
    chunk_z,
    local_x,
    local_z,
    surface_y,
    water_y,
    terrain_kind,
    source_macro_elevation,
    source_combined_macro_height,
    source_ocean_mask,
    source_lake_mask,
    source_coast_mask,
    source_dry_basin_mask,
    source_ridge_influence,
    source_terrain_ruggedness,
    source_river_valley_strength,
    source_river_flow_hint,
    source_meso_delta_blocks,
    source_meso_material_hint,
}
```

`PixelizedChunkArea.columns`는 입력 `MacroFieldTile.samples`의 row-major order를 유지한다. chunk-aligned
tile에서는 이 순서가 world z/x, 즉 chunk z/x와 local z/x로 안정적으로 해석된다. 같은 `(seed,
generator_version, chunk range, MacroFieldTile, PixelizeConfig)`은 같은 column sequence를 만들어야 한다.
`source_terrain_ruggedness`는 macro field의 final biome context ruggedness를 보존하며, downstream
heightfield shoreline/bevel policy가 stage 12 handoff 뒤에도 같은 ruggedness context를 읽을 수 있게 한다.
`source_meso_*` 계열 값은 stage 10 meso feature geometry를 다시 읽은 결과가 아니라, stage 11
macro_field가 이미 sample에 bake한 contribution/hint를 보존한 것이다.

---

## 입력 계약

`pixelize`가 소비하는 `MacroFieldTile`은 요청 chunk area를 정확히 덮는 chunk-aligned footprint여야 한다.

- tile origin은 최소 chunk의 world block origin과 일치한다.
- tile width/height는 `chunk_count * CHUNK_EDGE`와 일치한다.
- tile sample spacing은 `1.0` world block이다.
- 각 sample은 하나의 final voxel column 후보가 된다.

overview용 `macro_field_preview`처럼 더 넓은 footprint를 낮은 density로 샘플한 tile은 pixelize 입력이
아니다. 그런 tile은 stage 11 preview surface이며, stage 12 column resolve를 대표하지 않는다.

---

## Height / Water Resolve

height conversion은 `macro_field`, `pixelize`, `heightfield`가 공유하는 signed sea-level block-domain을
사용한다.

```text
combined_macro_height -0.50 -> -1024 blocks
combined_macro_height  0.00 ->   0 blocks = sea level
combined_macro_height  1.00 -> 2048 blocks
```

`pixelize`는 이 연속 block height를 integer column output으로 snap해 `surface_y`를 만든다. launch
slice에서는 `heightfield`의 기존 contour lower-band policy와 같은 결과를 내야 하며, raw macro scalar는
`combined_macro_height`와 `macro_elevation`으로 보존한다.
ocean/lake/river mask가 없는 일반 land source는 sea level 아래에서도 `surface_y`를 `y = 0`으로 floor하지
않고, downstream heightfield와 같은 dry below-sea bed를 보존한다.

standing water는 source macro masks를 따른다. `ocean_mask` 또는 `lake_mask`가 standing-water threshold를
넘으면 `water_y = Some(sea_level_blocks)`가 된다. river water는 selected hydrology를 다시 풀지 않고,
`MacroFieldSample`의 river valley/bed/water hint를 읽어 optional water hint로만 옮긴다. dry basin은
water mask가 아니며 `water_y`를 만들지 않는다.
Concave river cusp cleanup is not owned here. Stage 10 `macro_field` performs bounded river raster
cleanup before samples become `MacroFieldTile`, so `pixelize` must preserve the supplied source river
strength and only apply the shared heightfield threshold during column conversion.

---

## Runtime Cache

runtime generation cache chain은 pixelize 이후 chunk/voxel path가 graph나 macro source를 다시 묻지
않도록 아래 순서를 따른다.

```text
macro field tile cache
-> pixelized chunk area cache
-> heightfield / voxel-column realization cache
-> surface / vegetation plan
-> voxel fill writes ChunkData
```

`MacroFieldTileCache`는 stage 11 source field cache이고, `PixelizedChunkArea`는 stage 12 chunk-aligned
column cache다. chunk fill hot path는 graph/macro/hydrology/boundary를 다시 계산하지 않고 이 column
cache 또는 그 downstream heightfield/voxel-column output을 읽는다.

---

## Preview

`pixelize_preview`는 stage 12 output을 topdown chunk map으로 검사한다.

- positional input은 `<seed> <cx> <cz> <r>`이다.
- `cx/cz`는 chunk coordinate이고, `r`은 inclusive square chunk radius다.
- output footprint는 `(2r + 1) * CHUNK_EDGE` columns on each axis다.
- output PNG의 각 pixel은 정확히 하나의 `PixelizedColumn`을 의미한다.
- color ramp는 `surface_y`의 deterministic terrain height를 표시하고, water/dry/river/ridge hint는
  legend/metadata 또는 optional overlay/channel로 구분할 수 있어야 한다.
- PNG metadata/stdout은 chunk range, world block bounds, column resolution, sea level, height range,
  standing-water count, river-water hint count, source macro-field cache key를 기록한다.

preview는 stage 12의 layout contract를 확인하는 표면이다. preview renderer가 macro field를 직접
재샘플해서 column을 만들면 안 되며, 반드시 `PixelizedChunkArea`를 그려야 한다.

---

## 불변식

1. `pixelize`는 `MacroFieldTile`만 소비하며 graph/macro/hydrology/river-plan/boundary/meso-feature를 직접 다시
   해석하지 않는다.
2. 하나의 output pixel은 하나의 world block column이다.
3. output footprint는 chunk boundary에 정렬되어야 한다.
4. `surface_y`와 `water_y`는 integer block height다.
5. source `combined_macro_height`, macro masks, meso-baked source channel은 downstream stage가 진단/정책에 쓸 수 있게 보존된다.
6. 같은 입력 tile과 config는 같은 `PixelizedChunkArea`를 만든다.
7. 병렬 실행은 허용되지만 column order와 값은 scheduling에 의존하면 안 된다.
8. `heightfield`는 새 path에서 first pixel/column resolve를 다시 수행하지 않고 `PixelizedColumn`을
   downstream input으로 소비해야 한다.

---

## 현재 구현 상태

- `src/world/generation/pixelize/mod.rs`는 `MacroFieldTile`을 `PixelizedChunkArea`로 변환한다.
- `sample_spacing_blocks = 1.0`, integer world-block sample position, chunk-aligned origin, whole-chunk
  width/height를 runtime assert로 고정한다.
- conversion은 Rayon으로 병렬화되지만, output vector는 입력 sample order와 같은 deterministic order를
  유지한다.
- height resolve는 현재 `heightfield_column_from_sample`을 공유해 기존 heightfield compatibility
  vertical slice와 같은 `surface_y` / `water_y` 결과를 낸다.
- river concave-cusp cleanup now belongs to `macro_field`; area conversion does not mutate source
  river strengths or promote additional river water hints after the parallel per-sample conversion.
- 기존 `heightfield` 구현은 아직 `MacroFieldTile`을 직접 읽는 compatibility vertical slice다.
  `PixelizedColumn.surface_y` / `water_y`는 이 per-column compatibility resolve의 hint이며,
  `generate_heightfield_tile`이 area-level river descent 같은 후처리를 적용한 뒤 생성한
  `SurfacePlanArea`와는 다를 수 있다. surface-aware voxel fill은 position/footprint는
  `PixelizedChunkArea`에서, 최종 height/water/material은 `SurfacePlanArea`에서 읽는다. 다음 rewrite
  단계에서는 `PixelizedChunkArea` / `PixelizedColumn`을 소비하도록 옮긴다.
