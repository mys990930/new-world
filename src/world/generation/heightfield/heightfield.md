# heightfield

## 역할

`heightfield`는 graph-first generator의 11단계인 heightfield / water surface 합성 계약을 소유한다.

현재 구현은 vertical slice다. stage 8 `macro_field`가 만든 `MacroFieldTile`을 읽어 column-oriented
heightfield cache로 바꾸며, stage 9 meso feature와 stage 10 Perlin micro relief는 아직 값을 더하지
않는 stub으로 둔다.

```text
MacroFieldTile
-> meso_delta = 0
-> micro_relief = 0
-> HeightfieldTile / HeightfieldColumn
```

이 단계는 아직 final surface material이나 `ChunkData` voxel fill을 결정하지 않는다. 다만 이후 voxel
fill이 읽을 수 있는 surface height, water level, terrain kind hint, macro mask를 column 단위로 제공한다.

---

## 책임

- `MacroFieldTile` sample을 world-space column으로 변환한다.
- `combined_macro_height`를 block-space surface height로 매핑한다.
- ocean/lake mask에서 water level과 water column hint를 만든다.
- river valley, ridge, dry basin, water mask를 diagnostic terrain kind hint로 보존한다.
- meso/perlin stub 값이 0임을 데이터와 문서에 명시한다.
- column conversion은 deterministic하고 병렬 실행 순서에 영향을 받지 않아야 한다.

---

## 비책임

- meso feature 생성
- Perlin/fBM micro relief 생성
- biome/material resolve
- vegetation placement
- final `ChunkData` fill
- renderer/GPU 리소스 생성

---

## 공개 API

```rust
HeightfieldConfig::default()
generate_heightfield_tile(&MacroFieldTile, HeightfieldConfig) -> HeightfieldTile
heightfield_column_from_sample(&MacroFieldSample, HeightfieldConfig) -> HeightfieldColumn
```

주요 데이터:

```rust
HeightfieldConfig {
    sea_level_blocks,
    min_height_blocks,
    max_height_blocks,
    normalized_min_height,
    normalized_max_height,
    river_water_threshold,
    ocean_bed_blocks,
    lake_bed_blocks,
    shore_ramp_blocks,
    shore_min_land_blocks,
}

HeightfieldColumn {
    position,
    surface_height_blocks,
    surface_y,
    water_level_blocks,
    water_y,
    terrain_kind,
    macro_elevation,
    combined_macro_height,
    ocean_mask,
    lake_mask,
    dry_basin_mask,
    coast_mask,
    ridge_influence,
    river_valley_strength,
    river_flow_hint,
    meso_delta_blocks,
    micro_relief_blocks,
}
```

---

## Height Mapping

현재 launch preview scale은 block-space 진단용 매핑이다. 실제 meter 단위 terrain scale은 final
generator version에서 조정될 수 있지만, sea level은 pipeline 계약대로 world-space `y = 0`을 유지한다.

```text
combined_macro_height -0.75 -> -48 blocks
combined_macro_height  1.25 -> 160 blocks
sea level                    ->   0 blocks
```

`combined_macro_height`는 이미 macro elevation, ridge raise, river valley carve, coast/lake flatten을
합친 pre-Perlin 값이다. 따라서 heightfield stage는 river carve를 다시 강하게 중복 적용하지 않는다.
river 정보는 water hint와 terrain kind hint로 보존하고, 실제 channel carve/water body 폭은 후속
surface/voxel 단계에서 확정한다.

heightfield column의 최종 surface/water output은 voxel fill이 바로 읽을 수 있도록 integer block
height로 snap한다. launch 구현은 float macro value를 block-space로 변환한 뒤 `round`로 snap하고,
`surface_height_blocks == surface_y as f32` 관계를 유지한다. raw macro 값은 `macro_elevation`과
`combined_macro_height` diagnostic field에 남는다.

---

## Water Policy

- `ocean_mask > 0.5` 또는 `lake_mask > 0.5`이면 water level은 `sea_level_blocks`다.
- ocean/lake column의 terrain surface는 water bed로 취급하며, preview vertical slice에서는 기본적으로
  ocean bed를 `sea_level - 12 blocks`, lake bed를 `sea_level - 2 blocks` 이하로 낮춘다. 이는 final
  bathymetry가 아니라 수면과 지형 bed를 분리해 preview/voxel fill이 물을 볼 수 있게 하는 launch
  정책이다.
- 바다 수면은 `y = 0`이지만, 바다와 맞닿은 land column이 즉시 높은 vertical cliff가 되면 안 된다.
  explicit cliff/ridge/meso feature가 생기기 전까지 coast-adjacent land는 `coast_mask`를 읽어
  해수면에서 완만히 올라가는 shoreline ramp로 clamp한다. coast 바로 옆 land는 `y = 0` 근처에서
  시작하고, 내륙으로 갈수록 원래 macro height를 회복한다.
- 지형 surface가 water level보다 낮으면 water column이 생긴다.
- `river_valley_strength >= river_water_threshold`인 column은 `River` hint가 될 수 있지만, 현재 vertical
  slice에서는 height를 추가로 깎지 않는다.
- dry basin은 water가 아니다. `dry_basin_mask`는 `DryBasin` hint로 보존되지만 water level을 만들지 않는다.

---

## Runtime Cache

`HeightfieldTile`은 chunk fill hot path가 읽는 cache surface다.

```text
macro field tile cache
-> heightfield cache
-> chunk generation samples column/window data
-> voxel fill writes ChunkData
```

초기 구현에서는 preview binary가 하나의 macro field tile과 heightfield tile을 직접 생성한다. 런타임
연결 시에는 같은 계약을 worker cache miss로 옮겨야 하며, chunk fill은 graph/macro/hydrology/boundary를
반복 query하지 않는다.

---

## Preview

`heightfield_preview`는 이 모듈의 column을 진단용 isometric column surface로 바꿔 PNG를 만든다.

- 실제 `ChunkData` final fill이 아니다.
- block color는 final material이 아니라 terrain meaning 확인용 diagnostic ramp다.
- water/ocean은 muted blue, low land는 green-gray, high/ridge는 pale gray, dry basin은 muted
  gray/mauve 계열로 표시한다.
- 기본 preview는 offscreen 3D camera가 아니라 2D isometric projection을 직접 사용한다.

```text
screen_x = (x - z) * tile_w / 2
screen_y = (x + z) * tile_h / 2 - y * vertical_px_per_block
```

- `vertical_px_per_block`은 현재 heightfield의 min/max surface range와 이미지 높이를 읽어 자동
  산정한다. 기본 목표는 projected height span이 이미지 높이의 약 20-35%를 차지하는 것이다. 이 범위는
  top surface가 평면처럼 죽지 않고, 동시에 side wall이 preview를 지배하지 않도록 하기 위한 진단용
  계약이다.
- `--vertical-scale`은 자동 산정값에 곱해지는 preview-only multiplier이며 기본은 `1.0`이다. 실제
  heightfield 값을 바꾸지 않는다.
- column은 top diamond와 보이는 east/south side face만 그린다. 모든 column을 전역 base plane까지
  벽으로 내리면 side view처럼 보이기 때문에, 기본 preview는 neighbor height 차이를 보여주는
  terraced relief를 우선한다.
- preview overlay는 실제 world-block 맥락을 함께 표시한다. legend/header는 `center-x/center-z`,
  column resolution, sample spacing, chunk range/radius, world footprint, sea level, height range를
  기록해야 한다. chunk boundary는 `CHUNK_EDGE` block 간격의 별도 선으로, macro field tile/cache
  boundary는 graph/cache region scale의 별도 선으로 표시한다.
- meso/perlin stub이므로 fine grain이 보이면 macro field 또는 preview lighting/mesh artifact를 먼저
  의심한다.

---

## 불변식

1. 같은 `MacroFieldTile`과 `HeightfieldConfig`는 같은 `HeightfieldTile`을 만든다.
2. column count는 macro field sample count와 일치한다.
3. 모든 height와 mask 값은 finite여야 한다.
4. water mask가 있는 column은 water level hint를 가져야 한다.
5. meso/perlin stub 값은 현재 항상 0이다.
6. heightfield는 `macro_field`를 source로 읽으며 graph/macro/hydrology/boundary를 직접 재해석하지 않는다.
7. final column surface/water height는 integer block height로 snap되어야 한다.
8. coast-adjacent land는 explicit cliff feature가 없는 한 sea level에서 완만히 올라가야 하며, ocean
   water surface 바로 옆에 높은 vertical land wall을 만들면 안 된다.
