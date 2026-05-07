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

heightfield는 contour segment를 새로운 terrain source로 재구성하지 않는다. source of truth는 여전히
`MacroFieldTile.samples[].combined_macro_height`지만, 현재 experimental vertical slice의 final land
surface는 이 연속 scalar를 직접 쓰지 않는다. raw scalar는 diagnostic field로 보존하고, surface는
heightfield 직전 block-height domain에서 순수 contour band로 resolve한다. 즉 "등고선을 따라 생성"한다는
의미는 Marching Squares 선분을 다시 raster source로 쓰는 것이 아니라, column이 자신이 속한 contour
level의 계단식 block height를 최종 terrain surface로 사용한다는 뜻이다.

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
    horizontal_subdivisions,
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
    contour,
}

HeightfieldContourConfig {
    step_blocks,
    min_gap_blocks,
    river_min_gap_blocks,
    band_smoothing,
}

HeightfieldColumn {
    position,
    raw_surface_height_blocks,
    contour_guided_surface_height_blocks,
    constrained_surface_height_blocks,
    surface_height_blocks,
    surface_y,
    water_level_blocks,
    water_y,
    river_water_height_blocks,
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

`horizontal_subdivisions`는 X/Z 방향 sampling density 계약이다. 기본값 `1`은 입력
`MacroFieldTile`의 sample grid를 그대로 column으로 변환한다. preview나 runtime cache가 같은
world footprint를 더 촘촘히 보고 싶으면 macro field tile의 `width/height`를 각 축에서
`horizontal_subdivisions`배로 만들고 `sample_spacing_blocks`를 같은 비율로 줄인 뒤 heightfield로
넘긴다. 이 값은 heightfield 데이터의 vertical scale이 아니며, 같은 world-space sample과 같은 scalar는
subdivision 값과 무관하게 같은 integer `surface_y`/`water_y`를 가져야 한다.

---

## Height Mapping

현재 launch preview scale은 block-space 진단용 매핑이다. 실제 meter 단위 terrain scale은 final
generator version에서 조정될 수 있지만, sea level은 pipeline 계약대로 world-space `y = 0`을 유지한다.

```text
combined_macro_height -0.75 -> -64 blocks
combined_macro_height  0.00 ->   0 blocks = sea level
combined_macro_height  1.25 -> 224 blocks
```

이 매핑은 단일 선형 remap이 아니라 signed sea-level을 기준으로 한 piecewise remap이다. 음수
macro height는 `-0.75..0.0` 범위에서 `-64..0` block으로, 양수 macro height는 `0.0..1.25`
범위에서 `0..224` block으로 변환한다. 따라서 macro map의 coast-adjacent land가 `0` 근처의 signed
height를 가지면 해수면 `y = 0`에서 시작하며, 단순히 normalized range 중간값이라는 이유로 높은
terrace로 튀어서는 안 된다.

이 launch scale은 같은 world footprint 안에서 X/Z와 Y block resolution을 함께 2배 높인 block-domain
계약이다. preview 렌더링에서만 세로 비율을 속이는 것이 아니라, macro field contour 추출과
heightfield band resolve가 같은 block-height domain을 공유한다.

`combined_macro_height`는 이미 macro elevation, ridge raise, river valley carve, coast/lake flatten을
합친 pre-Perlin 값이다. 따라서 heightfield stage는 river carve를 다시 강하게 중복 적용하지 않는다.
river 정보는 water hint와 terrain kind hint로 보존하고, 실제 channel carve/water body 폭은 후속
surface/voxel 단계에서 확정한다.

입력 `MacroFieldTile`은 sea-level aligned coastal ramp를 제공해야 한다. 즉 connected ocean coast의
land-side scalar는 `0` 근처에서 시작하고 내륙으로 갈수록 회복되어야 한다. heightfield는 이 원천
scalar를 우회적으로 clamp해서 해안 단차를 숨기는 계층이 아니라, 이미 정렬된 macro scalar를 contour
step/integer block domain으로 옮기는 계층이다.

heightfield column은 값을 세 단계로 보존한다.

```text
raw_surface_height_blocks
  = combined_macro_height를 block-space로 변환한 연속 높이
contour_guided_surface_height_blocks
  = raw height가 속한 contour step의 lower band 높이
constrained_surface_height_blocks
  = sea-level water surface / shoreline contour ceiling / clamp를 적용한 snap 전 높이
surface_height_blocks
  = voxel fill이 바로 읽을 수 있게 contour step / integer block에 snap한 최종 높이
```

launch 구현은 최종 surface/water output을 integer block height로 snap하고,
`surface_height_blocks == surface_y as f32` 관계를 유지한다. raw macro 값은
`macro_elevation`과 `combined_macro_height` diagnostic field에도 남는다.

기본 contour band 설정은 heightfield stair-step 확인을 우선한다.

```text
contour.step_blocks = 1 block
contour.min_gap_blocks = 1 block
contour.river_min_gap_blocks = 1 block
contour.band_smoothing = 0.0
```

각 land column은 `raw_surface_height_blocks`가 속한 contour band의 lower level로 떨어진다. 기본
launch 정책은 raw 1-block band를 그대로 surface로 쓰지 않고, 다음 terrace로 올라가려면 raw
block-height가 `step_blocks + min_gap_blocks`, 즉 기본 2 blocks만큼 진행되어야 한다. 출력 높이는
여전히 `0, 1, 2, ...` integer step이다.

현재 기본값에서는 일반 land와 river corridor가 모두 1-block minimum gap을 사용한다. 다만
`river_min_gap_blocks` 필드는 유지한다. 이후 일반 land gap을 다시 넓히더라도 river corridor와
river-adjacent carve 영역은 hydrology/macro_field가 제공한 selected river valley strength와 display
flow hint를 읽어 더 작은 gap으로 override할 수 있어야 하기 때문이다. final river routing을
heightfield에서 다시 풀지는 않는다.
smoothing, smoothstep, band-local interpolation은 현재 사용하지 않는다. raw continuous height는
`raw_surface_height_blocks`와 `combined_macro_height`에 남지만 final terrain surface 결정에는 직접 쓰지 않는다.

---

## Water Policy

- `ocean_mask > 0.5` 또는 `lake_mask > 0.5`이면 water level은 `sea_level_blocks`다.
- ocean/lake column의 final visible surface는 water surface와 같은 `y = 0`이다. 이 vertical slice는
  ocean bathymetry를 만들지 않으며, `ocean_bed_blocks`와 `lake_bed_blocks`는 후속/debug bathymetry용
  설정으로만 남는다. preview나 heightfield visible top에 bed depression을 섞으면 회귀다.
- 일반 land column은 raw block height를 contour lower band로 양자화한 뒤 sea level 아래로 내려가지
  않는다. 즉 water가 아닌 terrain의 기본 floor는 `y = 0`이다.
- 바다/호수와 맞닿은 land column이 즉시 높은 vertical cliff가 되면 안 된다. tile 생성 후
  standing water(ocean/lake) column으로부터 grid distance를 계산하고, 주변 land에 shoreline contour
  ceiling을 적용한다. 이 pass는 continuous smoothing이 아니라 `0, 1, 2, ...` 계단 ceiling이다.
  water와 맞닿은 첫 land ring은 `y = 0`, 다음 ring은 `y = 1`, 그 다음은 `y = 2`처럼 contour step
  단위로만 올라간다.
- water-adjacent safety pass는 ocean/lake 같은 standing water만 기준으로 삼는다. river water hint를
  shoreline ocean/lake ramp 기준으로 사용하지 않는다.
- `river_valley_strength >= river_water_threshold`인 column은 `River` hint가 될 수 있다. river column은
  integer river water height를 갖고, 인접 river/standing-water surface와 비교해 한 column 이웃 사이에서
  한 block보다 크게 급락하지 않도록 preliminary descent pass를 적용한다. 이 pass는 full hydrology
  water surface solve가 아니라 stage 11 vertical slice용 안전 장치다.
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

- `vertical_px_per_block`은 preview 렌더링 전용 투영 값이지만, XZ density에 맞춰 따로 눌러지는 보정
  계수가 아니다. `heightfield_preview`는 block primitive가 화면에서 정육면체에 가깝게 읽히도록
  `vertical_px_per_block == tile_h_px`인 cubic scale로 그린다.
- 고정 XZ scale `4`에 대응하는 macro relief는 preview 렌더링이 아니라
  `macro_field`/`heightfield`가 공유하는 block-height 변환이 소유한다. 현재 launch scale은
  `-0.75..0.0..1.25 -> -64..0..224 blocks`이며 preview에서 같은 Y 값을 다시
  낮춰 그리면 중복 압축이다.
- heightfield preview의 XZ scale은 사용자 CLI 옵션이 아니다. 고정값 `4`는 같은 world footprint에서
  base column 대비 각 축 column 수를 네 배로 만들며, effective sample spacing은 1/4이 된다. preview는 이 산출 `y` height
  block, sea level, contour step, river water descent 값을 cubic block scale로 렌더한다.
  X/Z 렌더링 픽셀 스케일도 별도 옵션이 아니라 effective column count, footprint, image size에서
  자동으로 파생된다.
- column은 top diamond와 현재 `--quarter-turns` projection에서 보이는 side face만 그린다. 모든 column을
  전역 base plane까지 벽으로 내리면 side view처럼 보이기 때문에, 기본 preview는 neighbor height 차이를
  보여주는 terraced relief를 우선한다. quarter view가 바뀌면 painter order와 visible side도 함께 바뀌어야 한다.
- preview overlay는 실제 chunk/world-block 맥락을 함께 표시한다. legend/header는 positional
  `center-x/center-z`가 chunk coordinate임을 기본 계약으로 기록하고, 내부에서 변환한 center chunk,
  center world block, column resolution, sample spacing, chunk range/radius, world footprint, sea level,
  height range를 기록해야 한다. `heightfield_preview`의 주 grid는 `macro_field_preview`와 같은 1024-block
  macro field tile/cache boundary다. chunk boundary는 `CHUNK_EDGE` block 간격의 very faint minor
  line으로 유지하고, `CHUNK_EDGE * 8`인 256-block major grid는 보조 chunk-group reference로 더
  약하게 표시한다. 1024-block macro tile line이 terrain scale을 읽는 primary overlay여야 한다.
- `heightfield_preview`의 positional `center-x/center-z`는 기본적으로 chunk coordinate다.
  `--chunk-radius r`을 받으면 해당 center chunk를 중심으로 `center_chunk-r .. center_chunk+r`
  inclusive square range를 샘플링한다. 이 모드에서 `width/height`는 이미지 해상도만 정하고,
  world footprint는 chunk square가 정한다. 예전 world-block 입력이 필요하면 preview-only
  `--world-center`/`--world-coordinates` 호환 옵션을 사용한다.
- preview legend는 고정 픽셀 크기가 아니라 출력 이미지 크기에 비례해야 한다. 기본 metadata panel은
  화면 높이의 약 1/5을 목표로 하며, 글꼴, swatch, scale bar도 같은 비율로 커져야 한다.
- preview는 방향 compass overlay를 포함한다. topdown preview는 이미지 위=N, 오른쪽=E 기준을
  유지하지만, `heightfield_preview`의 compass는 isometric `--quarter-turns` projection 이후의
  screen-space 방향을 따른다. 따라서 N/E/S/W label은 현재 quarter view에서 실제 world cardinal
  방향이 화면에 놓이는 방향을 가리킨다.
- `--block-lines`는 각 column top/visible side polygon에 매우 얇은 diagnostic outline을 더한다.
  기본 preview에서는 켜져 있으며, terrain 색을 압도하면 `--no-block-lines`로 끌 수 있다.
  outline은 top face 외곽선과 visible side face 외곽선뿐 아니라 side face의 정수 `y` step마다
  아주 얇은 horizontal guide를 그려, 작은 `--chunk-radius 1` preview에서도 개별 block 층을 읽을 수
  있어야 한다. 이 선은 final mesh edge가 아니라 preview 전용 scale guide이며 terrain/water 색보다
  약하게 보여야 한다.
- preview metadata/stdout과 legend는 contour-band heightfield mode, contour step, minimum gap,
  smoothing disabled 값을 기록해야 한다.
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
9. heightfield contour band resolve는 raw macro scalar를 diagnostic으로 보존하되 final land terrain
   surface에는 직접 쓰지 않는다. water/shoreline constraint 전의 land column은 자신이 속한 contour
   band의 lower height가 되어야 하며, smoothing/interpolation을 적용하면 안 된다.
10. water-adjacent visible top은 bed가 아니라 water surface `y = 0`과 비교해야 한다. 순수
    contour-step mode에서 water와 맞닿은 land ring은 `y = 0`부터 시작하고, shoreline ramp 안쪽으로
    갈수록 contour step 단위로만 올라가야 한다.
11. ocean/lake visible surface는 항상 `y = 0`이며, preview vertical slice에서 ocean side가 깊게
    파인 지형처럼 보이면 회귀다.
12. river water hint는 integer block height이며, 인접 river/standing-water pair에서 큰 급락을 만들지
    않아야 한다. 현재 구현은 neighbor delta를 한 block 이하로 제한하는 preliminary descent pass다.
13. contour gap 정책은 final height를 렌더링으로 속이는 값이 아니라 heightfield band resolve 계약이다.
    현재 기본값은 `step_blocks = 1`, 일반 `min_gap_blocks = 1`, `river_min_gap_blocks = 1`이며,
    raw height가 2 block 진행될 때마다 visible terrain은 1 integer step 올라간다. river corridor
    override 구조는 남기지만 기본값은 land와 river가 같다. sea level `y=0`, shoreline ceiling,
    river descent는 이 snap 결과 위에서 유지되어야 한다.
14. horizontal subdivision은 X/Z column density와 sample spacing만 바꾸며, 그 자체가 per-sample
    vertical rescale knob가 아니다. fixed XZ scale `4`에 맞춘 launch relief는 shared
    block-height domain의 `-64..224` 기본 범위가 소유한다. preview 렌더러는 산출된 `surface_y`와
    water hint를 cubic block scale로 그려야 하며, subdivision 값으로 같은 Y 값을 다시 낮춰 보이면
    중복 압축이다.
