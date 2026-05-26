# heightfield

## 역할

`heightfield`는 graph-first generator의 13단계 column realization 계약을 소유한다.
target path에서는 stage 12 `pixelize`가 만든 `PixelizedChunkArea` / `PixelizedColumn`을 소비하고,
현재 compatibility path에서는 `MacroFieldTile`을 직접 읽어 `HeightfieldTile`을 만든다.

현재 river realization은 reset 상태다. `heightfield`는 river hint를 terrain carve, water solve,
terrain kind 판정에 사용하지 않는다. river 관련 public fields는 호환을 위해 남아 있지만 column resolve
결과에서는 neutral 값으로 정리된다.

---

## 책임

- macro/pixelized source height를 block-space raw/contour/constrained/final surface height로 변환한다.
- ocean/lake water level과 standing-water column hint를 만든다.
- coast/ridge/dry basin/meso/perlin diagnostic fields를 column에 보존한다.
- optional Perlin micro relief를 적용한다.
- tile stats와 neighbor water/visible-step diagnostics를 계산한다.
- accidental `RiverCore` / `RiverBed` column이나 river hint가 남아 있으면 reset guard에서 제거한다.

---

## 비책임

- river valley, shoulder, bed, core, estuary fan carve
- river water descent, Q-based water depth, active river core solve
- hydrology/river-plan topology 해석
- meso feature geometry 재해석
- biome/material/vegetation resolve
- final `ChunkData` voxel fill

---

## River Reset 계약

- `heightfield_column_from_sample`은 `MacroFieldSample`의 river/estuary hints를 읽어 terrain을 바꾸지 않는다.
- `HeightfieldTerrainKind::RiverCore`와 `HeightfieldTerrainKind::RiverBed`는 현재 생성하지 않는다.
- `river.rs`의 post-pass는 compatibility guard다. 모든 river strengths, flow/depth/roughness hints,
  `river_core_water_height_blocks`를 neutral로 만들고 accidental river terrain kind를 land로 돌린다.
- ocean/lake water policy는 계속 동작한다. river hint가 ocean/lake water를 override하면 회귀다.
- Perlin은 river-specific relief를 만들지 않는다. river hint가 있는 land sample도 ordinary land relief를 따른다.

---

## 공개 API

```rust
HeightfieldConfig::default()
generate_heightfield_tile_from_pixelized_area(&PixelizedChunkArea, HeightfieldConfig) -> HeightfieldTile
heightfield_column_from_pixelized_column(&PixelizedColumn, HeightfieldConfig) -> HeightfieldColumn
```

Compatibility wrapper:

```rust
generate_heightfield_tile(&MacroFieldTile, HeightfieldConfig) -> HeightfieldTile
heightfield_column_from_sample(&MacroFieldSample, HeightfieldConfig) -> HeightfieldColumn
```

---

## 파일 구조

- `mod.rs`: public type/config/facade API and leaf module exports.
- `mapping.rs`: normalized macro scalar to block height conversion, contour/snap helpers.
- `column.rs`: single-sample to `HeightfieldColumn` conversion and water helper orchestration.
- `water.rs`: ocean/lake water-level and standing-water bed helpers.
- `river.rs`: river reset guard only.
- `perlin.rs`: optional deterministic micro relief.
- `stats.rs`: tile diagnostics and neighbor traversal helpers.
- `column_tests.rs`: module-local regression tests.

---

## Height Mapping

Sea level is world-space `y = 0`.

```text
combined_macro_height -0.50 -> -1024 blocks
combined_macro_height  0.00 ->     0 blocks
combined_macro_height  1.00 ->  2048 blocks
```

The final terrain surface is snapped to integer block height after contour-band resolve. Raw source
height is preserved for diagnostics as `raw_surface_height_blocks`; voxel fill should use the final
integer `surface_y` / `water_y`.

---

## Water Policy

- Ocean-owned columns receive sea-level water only when the resolved terrain bed is below sea level.
- Ocean-owned terrain at or above sea level keeps its source bed and has no water column.
- Lake columns use lake water helpers and do not depend on river hints.
- Land/coast/dry/ridge columns do not create water solely from negative terrain unless their explicit
  ocean/lake/coast policy allows it.
- River hints never create or raise water in the reset state.
