# heightfield

## 역할

`heightfield`는 graph-first generator의 12단계인 heightfield / voxel-column realization rewrite 계약을 소유한다.

새 stage contract에서 first chunk-aligned pixel/column resolve는 stage 11 `pixelize`가 소유한다.
`heightfield`는 `PixelizedChunkArea` / `PixelizedColumn`을 downstream input으로 소비하며,
`MacroFieldTile`을 직접 resample해 world-space column을 처음 만드는 책임을 갖지 않는다.

현재 구현은 compatibility vertical slice다. 아직 stage 10 `macro_field`가 만든 `MacroFieldTile`을
직접 읽어 column-oriented heightfield cache로 바꾸지만, 이 path는 pixelize module이 들어오기 전까지의
임시 wrapper다. rewrite 후에는 같은 height/water policy를 pixelized columns 위에 적용한다.

```text
MacroFieldTile
-> PixelizedChunkArea / PixelizedColumn
-> meso_delta = 0
-> optional Perlin relief
-> HeightfieldTile / HeightfieldColumn / voxel-column realization
```

이 단계는 아직 final surface material이나 `ChunkData` voxel fill을 결정하지 않는다. 다만 이후 voxel
fill이 읽을 수 있는 surface height, water level, terrain kind hint, macro mask를 column 단위로 제공한다.

heightfield는 contour segment를 새로운 terrain source로 재구성하지 않는다. source of truth는
`PixelizedColumn`이 보존한 source `combined_macro_height`와 integer column resolve다. 현재 vertical
slice의 final land surface는 이 연속 scalar를 직접 쓰지 않는다. raw scalar는 diagnostic field로 보존하고, surface는
heightfield 직전 block-height domain에서 순수 contour band로 resolve한다. 즉 "등고선을 따라 생성"한다는
의미는 Marching Squares 선분을 다시 raster source로 쓰는 것이 아니라, column이 자신이 속한 contour
level의 계단식 block height를 최종 terrain surface로 사용한다는 뜻이다.

---

## 책임

- `PixelizedColumn`을 downstream heightfield / voxel-column cache로 변환한다.
- pixelize가 보존한 `combined_macro_height`와 integer `surface_y`를 block-space surface policy로 연결한다.
- lake mask와 sea level 아래 ocean bed에서 water level과 water column hint를 만든다.
- river valley, river bed hint, ridge, dry basin, water mask를 diagnostic terrain kind hint로 보존한다.
- raw `coast_mask`는 column data로만 보존한다. 현재 baseline에서 coast mask는 별도 heightfield
  terrain kind, water column, shallow shelf, shoreline bevel, land-side ramp를 만들지 않는다.
- meso 값이 0임을 데이터와 문서에 명시한다.
- selected river bed와 bank/shoulder에는 world-space deterministic relief를 기본 적용해 완전히 균일한
  계단식 bed를 줄인다. 이 relief는 terrain bed만 움직이고 river/ocean/lake water surface continuity를
  소유하거나 변경하지 않는다. bank/shoulder relief는 selected river 주변의 finite distance와 strong
  near-bank valley signal이 있는 좁은 band에만 적용하며, broad valley 주변 지형을 macro noise처럼
  흔들면 안 된다.
- Perlin relief는 `HeightfieldPerlinConfig.enabled`일 때만 적용하며 기본값은 비활성화다.
- column conversion은 deterministic하고 병렬 실행 순서에 영향을 받지 않아야 한다.

---

## 비책임

- meso feature 생성
- macro-scale Perlin/fBM terrain ownership
- biome/material resolve
- vegetation placement
- final `ChunkData` fill
- renderer/GPU 리소스 생성
- first chunk-aligned `1 world block = 1 pixel = 1 voxel column` resolve

---

## 공개 API

```rust
HeightfieldConfig::default()
generate_heightfield_tile_from_pixelized_area(&PixelizedChunkArea, HeightfieldConfig) -> HeightfieldTile
heightfield_column_from_pixelized_column(&PixelizedColumn, HeightfieldConfig) -> HeightfieldColumn
```

Compatibility wrapper until the pixelize module lands:

```rust
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
    contour,
    perlin,
}

HeightfieldPerlinConfig {
    enabled,
    seed,
    generator_version,
    amplitude_blocks,
    base_scale_blocks,
    octaves,
    persistence,
    lacunarity,
    max_abs_blocks,
}

HeightfieldContourConfig {
    step_blocks,
    min_gap_blocks,
    river_min_gap_blocks,
    band_smoothing,
}

HeightfieldColumn {
    position,
    chunk,
    local,
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
    terrain_ruggedness,
    river_valley_strength,
    river_flow_hint,
    river_bed_depth_blocks,
    river_bank_roughness_hint,
    river_gravel_hint,
    river_cutbank_hint,
    meso_delta_blocks,
    micro_relief_blocks,
}
```

Heightfield는 별도 수평 scale 값을 소유하지 않는다. X/Z 방향 해상도와 chunk/local layout은 입력
`PixelizedChunkArea`가 직접 정의한다. stage 11의 handoff density는
`1 world block = 1 pixel = 1 voxel column`이며, heightfield가 이 density를 다시 해석하거나
resample하면 안 된다. 같은 `PixelizedColumn`과 config는 같은 integer `surface_y`/`water_y`를 가져야 한다.

---

## Height Mapping

현재 height mapping은 macro field, pixelize, heightfield, pixel/column preview가 공유하는 block-space 계약이다.
generator version이 의도적으로 바뀌지 않는 한 sea level은 world-space `y = 0`을 유지한다.

```text
combined_macro_height -0.50 -> -1024 blocks
combined_macro_height  0.00 ->   0 blocks = sea level
combined_macro_height  1.00 -> 2048 blocks
```

이 매핑은 단일 선형 remap이 아니라 signed sea-level을 기준으로 한 piecewise remap이다. 음수
macro height는 `-0.5..0.0` 범위에서 `-1024..0` block으로, 양수 macro height는 `0.0..1.0`
범위에서 `0..2048` block으로 변환한다. 현재 관심 구간 `-0.25..0.75`는 같은 기울기에서
`-512..1536 blocks`로 매핑된다. 따라서 macro map의 coast-adjacent land가 `0` 근처의 signed height를
가지면 해수면 `y = 0`에서 시작하며, 단순히 normalized range 중간값이라는 이유로 높은 terrace로
튀어서는 안 된다. effective range 바깥 값은 block conversion에서 `-1024` 또는 `2048` block으로
포화된다.

이 scale은 preview 렌더링에서만 세로 비율을 속이는 값이 아니라, macro field contour 추출,
pixelize integer column resolve, heightfield band resolve가 공유하는 block-height domain이다.

`combined_macro_height`는 이미 macro elevation, ridge raise, broad river valley, lake flatten을
합친 pre-Perlin 값이다. 따라서 pixelize/heightfield stage는 river channel carve를 다시 강하게 중복 적용하지 않는다.
river plan에서 온 narrow river bed 정보는 water hint와 terrain kind hint로 보존하고, 실제
channel carve/water body 폭은 후속 surface/voxel 단계에서 확정한다.

입력 `PixelizedColumn`은 stage 10 `MacroFieldTile`에서 온 source macro scalar를 보존해야 한다.
connected ocean coast의 land-side scalar가 낮은 양수에서 시작한다면 그 source는 `macro_map`의 coastal
elevation ramp와 ordinary boundary blend다. heightfield는 이 원천 scalar를 우회적으로 clamp해서 해안
단차를 숨기는 계층이 아니라, pixelize가 이미 정렬한 column output을 downstream contour/voxel-column
policy로 옮기는 계층이다.

heightfield column은 값을 세 단계로 보존한다.

```text
raw_surface_height_blocks
  = combined_macro_height를 block-space로 변환한 연속 높이
contour_guided_surface_height_blocks
  = raw height가 속한 contour step의 lower band 높이
constrained_surface_height_blocks
  = sea-level water surface policy와 min/max clamp를 적용한 snap 전 높이
surface_height_blocks
  = voxel fill이 바로 읽을 수 있게 contour step / integer block에 snap한 최종 높이
```

launch 구현은 최종 surface/water output을 integer block height로 snap하고,
`surface_height_blocks == surface_y as f32` 관계를 유지한다. raw macro 값은
`macro_elevation`과 `combined_macro_height` diagnostic field에도 남는다.

기본 contour band 설정은 heightfield stair-step 확인을 우선한다.

```text
contour.step_blocks = 1 block
contour.min_gap_blocks = 0 blocks
contour.river_min_gap_blocks = 0 blocks
contour.band_smoothing = 0.0
```

각 land column은 `raw_surface_height_blocks`가 속한 contour band의 lower level로 떨어진다. 기본
launch 정책은 raw block-height와 visible block-height가 같은 scale을 갖도록 `min_gap_blocks = 0`을
사용한다. 즉 raw `0.0..0.999`는 `y = 0`, raw `1.0..1.999`는 `y = 1`처럼 integer lower band로
snap된다.

현재 기본값에서는 일반 land와 river corridor가 모두 0-block minimum gap을 사용한다. 다만
`river_min_gap_blocks` 필드는 유지한다. 이후 일반 land gap을 넓히더라도 river corridor와
river-adjacent carve 영역은 river_plan/macro_field가 제공한 selected river bed hint, valley strength,
display flow hint를 읽어 더 작은 gap으로 override할 수 있어야 하기 때문이다. final river routing을
heightfield에서 다시 풀지는 않는다.
smoothing, smoothstep, band-local interpolation은 현재 사용하지 않는다. raw continuous height는
`raw_surface_height_blocks`와 `combined_macro_height`에 남지만 final terrain surface 결정에는 직접 쓰지 않는다.

---

## Water Policy

- `ocean_mask > 0.5`이면서 최종 terrain bed가 `sea_level_blocks` 아래이면 water level은
  `sea_level_blocks`다. ocean-owned bed가 sea level 이상이면 ocean terrain kind와 source bed는
  보존하지만 visible/active water column은 만들지 않는다.
- `lake_mask > 0.5`이면 water level은 절대 `y = 0` 고정값이 아니라 lake source macro elevation보다
  몇 block 낮은 shoreline-compatible level로 둔다. lake bed가 충분히 깎이지 않은 edge column에서는
  terrain이 수면보다 높게 남을 수 있으며, heightfield가 모든 lake column을 수면 아래로 강제 평탄화하지
  않는다.
- ocean column은 terrain bed와 water surface를 분리한다. connected ocean / ocean mask column의
  source bed는 heightfield terrain bed로 직접 보존하고, bed가 sea level 아래일 때만 `water_y`는 sea
  level `y = 0`으로 둔다.
  heightfield는 source bed가 sea level 이상으로 들어온 ocean column을 임의의 shallow fallback plane으로
  내리지 않는다. ocean source bed를 coast-adjacent sea level 근처에서 자연스럽게 이어지게 만드는 책임은
  앞 단계의 macro_field bathymetry가 갖는다. 기존처럼 terrain bed 자체를 `y = 0` 또는 단일 shallow
  plane으로 평면화하지 않는다. 일반 lake column도 water
  surface와 terrain bed를 분리하며, macro_field가 제공한
  U자형 lake bed height를 terrain surface로 보존한다. lake bed는 source raw bed를 따르되
  수면에서 과도하게 깊어지지 않도록 depth cap만 적용하고, 수면 바로 아래 완전 flat plane으로 덮어쓰면 안 된다. selected
  river corridor가 standing-water mask와 겹치는 하구/유출부 column은 terrain kind가 ocean/lake로 남더라도
  물 표면을 유지하면서 terrain bed를 river bed depth만큼 깎아 자연스럽게 연결할 수 있다.
- `coast_mask`는 heightfield baseline에서 diagnostic field다. ocean/lake/river/dry/ridge mask가 없는
  non-ocean land column은 coast mask가 있어도 일반 land와 같은 floor/snap 정책을 따른다. 따라서
  negative coast-mask land는 `surface_y = 0`, `water_y = None`으로 floor되고, near-zero positive
  coast-mask land도 deterministic shelf variation 없이 ordinary contour snap을 따른다.
- 일반 land column은 raw block height를 contour lower band로 양자화한 뒤 sea level 아래로 내려가지
  않는다. 즉 water가 아닌 terrain의 기본 floor는 `y = 0`이다. 이 floor는 connected ocean/ocean mask
  column의 terrain bed에는 적용하지 않는다.
- 일반 land에는 인접 column 기준 final surface ceiling, ocean shoreline bevel, land-side coast ramp를
  적용하지 않는다. 호수 bed/water 정책은 lake mask 내부에서만 처리한다.
- river water hint를 shoreline ocean/lake ramp 기준으로 사용하지 않는다.
- `river_valley_strength >= river_water_threshold`인 high-core column은 `River` hint가 될 수 있다.
  기본 threshold는 broad valley shoulder가 곧바로 물/강바닥으로 승격되지 않도록 높게 유지한다.
  river column은 macro_field의 bed-depth hint를 읽어 terrain bed를 water surface와 분리한다. 이 hint는
  river plan의 Q 기반 `bed_depth_blocks`에서 온 값이므로 heightfield가 다시 임의의 큰 상수로 증폭하지
  않는다. 상류 수원부는 얕은 1-block 안팎 stream과 작은 V-cut 감각으로 시작하고, 하류로 갈수록 Q에
  비례해 더 깊은 bed와 더 큰 water depth를 허용한다. ocean visible water surface는 여전히 `y = 0`이지만,
  river bed는 하구에서도 sea level 아래로 패일 수 있다.
  ocean-owned 또는 lake-owned mouth column은 ordinary river-water threshold보다 낮은 raster strength로
  들어와도 selected river `bed_depth_hint`가 남아 있으면 standing-water surface와 terrain bed를 분리해
  mouth bed carve를 적용한다. 이 예외는 water ownership을 새로 만들지 않고, 이미 전달된 selected river
  bed hint를 terrain bed에만 적용한다.
  high-core river bed와 adjacent bank/shoulder에는 deterministic value-noise relief를 기본 적용한다.
  bed relief는 water level 계산 뒤 terrain bed에만 들어가며, water depth 범위 안에서 clamp해 수면을
  뚫거나 한 column 이웃 river water continuity를 깨지 않는다. ocean-owned 또는 lake-owned mouth column도
  selected river bed hint가 있으면 같은 mouth carve를 적용하되 standing water surface는 그대로 유지한다.
  adjacent bank/shoulder relief는 broad river valley guide 전체가 아니라 finite river distance 안의
  near-bank band에만 들어간다. 낮은 `river_valley_strength`가 남아 있는 넓은 주변 지형은 macro_field의
  broad-valley lowering만 보존하고 heightfield-local noise를 추가하지 않는다.
  integer river water height는 별도 hint로 유지하고, 인접 river/standing-water surface와 비교해 한 column
  이웃 사이에서 한 block보다 크게 급락하지 않도록 preliminary descent pass를 적용한다. 이 pass는 full
  hydrology water surface solve가 아니라 stage 12 vertical slice용 안전 장치다.
- dry basin은 water가 아니다. `dry_basin_mask`는 `DryBasin` hint로 보존되지만 water level을 만들지 않는다.

---

## Runtime Cache

`HeightfieldTile`은 chunk fill hot path가 읽는 cache surface다.

```text
macro field tile cache
-> pixelized chunk area cache
-> heightfield cache
-> chunk generation samples column/window data
-> voxel fill writes ChunkData
```

초기 compatibility 구현에서는 preview binary가 하나의 macro field tile과 heightfield tile을 직접 생성할
수 있다. 런타임 연결 시에는 stage 11 pixelized chunk area cache를 worker cache miss로 준비하고,
heightfield는 그 column output을 소비해야 한다. chunk fill은 graph/macro/hydrology/river-plan/final-cell-context/boundary를
반복 query하지 않는다.

---

## Preview

`heightfield_preview`는 이 모듈의 column을 진단용 isometric column surface로 바꿔 PNG를 만든다.

- 실제 `ChunkData` final fill이 아니다.
- block color는 final material이 아니라 terrain meaning 확인용 diagnostic ramp다.
- active water/submerged ocean은 muted blue, low land는 green-gray, high/ridge는 pale gray, dry basin은
  muted gray/mauve 계열로 표시한다. ocean-owned terrain이라도 final bed가 sea level 이상이고 water
  column이 없으면 preview terrain pass는 land ramp 색을 사용한다.
- water/ocean/lake/river water는 지형/bed face를 먼저 그린 뒤 반투명 top/side overlay로 렌더한다.
  따라서 `y < 0` riverbed, lake bed, ocean bathymetry가 수면 아래에서도 진단 가능해야 한다.
- 기본 preview는 offscreen 3D camera가 아니라 2D isometric projection을 직접 사용한다.

```text
screen_x = (x - z) * tile_w / 2
screen_y = (x + z) * tile_h / 2 - y * vertical_px_per_block
```

- `vertical_px_per_block`은 preview 렌더링 전용 투영 값이지만, XZ density에 맞춰 따로 눌러지는 보정
  계수가 아니다. `heightfield_preview`는 block primitive가 화면에서 정육면체에 가깝게 읽히도록
  `vertical_px_per_block == tile_h_px`인 cubic scale로 그린다.
- macro relief는 preview 렌더링이 아니라 `macro_field`/`pixelize`/`heightfield`가 공유하는 block-height 변환이
  소유한다. 현재 shared height mapping은 effective
  `-0.5..0.0..1.0 -> -1024..0..2048 blocks`이며, 관심 구간 `-0.25..0.75`는 `-512..1536 blocks`다.
  preview에서 같은 Y 값을 다시 낮춰 그리면 중복 압축이다.
- heightfield preview의 X/Z 밀도는 scale 계층이 아니라 column count로 직접 표현한다. chunk-radius
  preview의 기본값은 chunk 하나를 `32`개 column으로 샘플링하고, free window preview는 기본
  `1024` block footprint를 `1024`개 X column으로 샘플링한다. 이 기본값은 cubic block
  render와 큰 Y block-domain에서 1 world block당 1 column에 해당한다. `--columns-x`/`--columns-z`가
  지정되면 그것이 최종 column count다.
  sample spacing은 world footprint / column count에서 파생된다. preview는 산출 `y` height block,
  sea level, contour step, river water descent 값을 cubic block scale로 렌더한다.
- column은 top diamond와 현재 `--quarter-turns` projection에서 보이는 side face만 그린다. 모든 column을
  전역 base plane까지 벽으로 내리면 side view처럼 보이기 때문에, 기본 preview는 neighbor height 차이를
  보여주는 terraced relief를 우선한다. quarter view가 바뀌면 painter order와 visible side도 함께 바뀌어야 한다.
- water overlay는 terrain/bed pass 뒤에 별도 deterministic painter order로 그린다. water side face는
  water surface와 bed 또는 인접 water surface 사이만 반투명으로 채워야 하며, opaque water top으로
  bed top을 대체하면 안 된다.
- heightfield preview는 scale diagnostic으로 형광색 player cube를 footprint 중앙에 그린다. 이 큐브는
  final gameplay entity가 아니며, world/block 기준 `1 x 1 x 4` block 크기만 확인하기 위한 preview
  marker다. 바닥은 중앙 `1 x 1` block footprint와 가장 가까운 heightfield column들의 `surface_y`
  최댓값에 맞춰 지형 블럭 위에 놓는다.
- preview overlay는 실제 chunk/world-block 맥락을 함께 표시한다. legend/header는 positional
  `center-x/center-z`가 chunk coordinate임을 기본 계약으로 기록하고, 내부에서 변환한 center chunk,
  center world block, column resolution, sample spacing, chunk range/radius, world footprint, sea level,
  height range를 기록해야 한다. `heightfield_preview`의 주 grid는 `macro_field_preview`와 같은 1024-block
  macro field tile/cache boundary다. chunk overlay는 preview footprint의 chunk-aligned 외곽선만
  표시하고, `CHUNK_EDGE` block 간격의 내부 minor grid는 표면 자글거림처럼 보일 수 있으므로 그리지
  않는다. `CHUNK_EDGE * 8`인 256-block major grid는 보조 chunk-group reference로 약하게 표시한다.
  1024-block macro tile line이 terrain scale을 읽는 primary overlay여야 한다.
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
- heightfield preview는 per-block face outline이나 side face의 정수 `y` guide line을 렌더하지 않는다.
  block scale 확인은 filled column faces, player diagnostic cube, world/grid reference overlay가 맡는다.
- 중앙 player diagnostic cube는 형광색 계열을 사용해 terrain diagnostic ramp와 명확히 구분한다.
  `--quarter-turns`에 따른 painter order와 visible side face 선택을 terrain column과 같은 isometric
  projection 규칙으로 따라야 한다.
- preview metadata/stdout과 legend는 contour-band heightfield mode, contour step, minimum gap,
  smoothing disabled 값을 기록해야 한다.
- preview metadata/stdout과 legend는 stage 11 pixelize handoff 여부와 source column count를 기록해야 한다.
- preview metadata/stdout과 legend는 player diagnostic cube의 `1 x 1 x 4` block dimensions, 중앙 world
  position, bottom/top `y`, sampled column count를 기록해야 한다.
- Perlin micro relief와 ocean bed Perlin relief는 `--perlin` preview flag 또는 명시적으로 enabled
  config를 전달한 경우에만 보인다. 기본 preview와 기본 `HeightfieldConfig`에서는
  `micro_relief_blocks = 0`이다. 다만 selected river bed와 adjacent bank/shoulder의 deterministic
  relief는 기본으로 켜져 있어 완전히 uniform한 river bed를 피한다. enabled config에서는 exact sea level
  border, Perlin max displacement depth 정도의 shallow ocean-owned below-sea border band, 그리고 그 이상
  ocean-owned dry terrain이 ordinary land와 같은 micro relief map을 사용한다. ocean bed relief는 sea
  level 아래 terrain bed에만 적용하며 sea-level water surface는 움직이지 않는다.

---

## 불변식

1. 같은 `PixelizedChunkArea`와 `HeightfieldConfig`는 같은 `HeightfieldTile`을 만든다.
2. column count는 pixelized column count와 일치한다.
3. 모든 height와 mask 값은 finite여야 한다.
4. lake mask가 있는 column과 sea level 아래 ocean bed column은 water level hint를 가져야 한다.
5. meso 값은 현재 항상 0이고, Perlin micro relief는 config가 disabled이면 항상 0이다.
6. heightfield는 `pixelize` output을 source로 읽으며 graph/macro/hydrology/river-plan/final-cell-context/boundary를 직접 재해석하지 않는다.
7. final column surface/water height는 integer block height로 snap되어야 한다.
8. `coast_mask`는 현재 heightfield baseline에서 diagnostic field다. coast mask만으로 water,
   terrain kind, shallow shelf, ocean shoreline bevel, land-side coast ramp를 만들면 회귀다.
9. heightfield contour band resolve는 raw macro scalar를 diagnostic으로 보존하되 final land terrain
   surface에는 직접 쓰지 않는다. land column은 자신이 속한 contour band의 lower height가 되어야 하며,
   smoothing/interpolation을 적용하면 안 된다.
10. water-adjacent visible top은 bed가 아니라 해당 water surface와 비교해야 한다. ocean은 `y = 0`이고
    lake는 lake source elevation에서 derive한 water level이다. 순수 contour-step mode에서
    heightfield는 ocean shoreline bevel이나 land ring clamp를 적용하지 않는다.
11. ocean visible water surface가 존재하면 항상 `y = 0`이지만 ocean terrain bed는 source heightfield
    bed로 분리된다. connected ocean column을 `surface_y = 0`으로 clamp하거나 sea-level 이상 source를
    heightfield-local shallow fallback plane으로 내리면 회귀다. sea-level 이상 ocean bed에는 water
    column을 만들지 않는다. coast-adjacent continuity와 shelf/slope/basin depth는 macro_field
    bathymetry source가 제공해야 하며, heightfield는 그 source bed를 직접 보존한다.
    ocean owner가 아닌 coast-mask negative land는 일반 non-water land처럼 `surface_y = 0`,
    `water_y = None`으로 남아야 한다. coast-connected near-zero positive land도 deterministic
    shallow-shelf variation 없이 ordinary contour snap을 따라야 한다.
    lake visible surface는 lake source elevation에서 derive한 water level이며, 일반 lake bed는 그
    아래의 U자형 terrain bed로 분리된다. lake water를 항상 `y = 0`에 고정하거나 lake bed를 완전 flat
    plane으로 만들면 회귀다.
12. river water hint는 integer block height이며, 인접 river/standing-water pair에서 큰 급락을 만들지
    않아야 한다. 현재 구현은 neighbor delta를 한 block 이하로 제한하는 preliminary descent pass다.
13. contour gap 정책은 final height를 렌더링으로 속이는 값이 아니라 heightfield band resolve 계약이다.
    현재 기본값은 `step_blocks = 1`, 일반 `min_gap_blocks = 0`, `river_min_gap_blocks = 0`이며,
    raw block height와 visible terrain은 같은 block scale을 유지한다. river corridor
    override 구조는 남기지만 기본값은 land와 river가 같다. sea level `y=0`와 river descent는 이 snap
    결과 위에서 유지되어야 한다.
14. X/Z column density는 입력 `PixelizedChunkArea`의 column count와 chunk/local layout이 직접 소유한다.
    별도 fixed scale이나 horizontal subdivision 값으로 heightfield Y를 해석하면 안 된다. launch relief는
    shared block-height domain의 `-1024..2048` 기본 범위가 소유한다. preview 렌더러는 산출된
    `surface_y`와 water hint를 cubic block scale로 그려야 하며, column density로 같은 Y 값을 다시
    낮춰 보이면 중복 압축이다.
15. 새 path에서 first chunk-aligned `1 world block = 1 pixel = 1 voxel column` resolve는 `pixelize`가
    소유한다. heightfield가 `MacroFieldTile`을 직접 resample해 이 resolve를 반복하면 회귀다.
16. Perlin micro relief는 기본적으로 꺼져 있다. preview용 enabled config는 현재
    `raw + micro -> contour -> snap/clamp` 순서를 사용해 contour stair-step이 지나치게 직접 보이는
    문제를 줄인다.
17. Perlin micro relief는 seed, generator version, world-space x/z로만 결정되어야 하며 chunk-local
    random state나 병렬 실행 순서에 의존하면 안 된다.
18. Lake와 genuinely submerged ocean source의 land micro relief는 0이고, river column
    `micro_relief_blocks`도 river continuity 보호를 위해 0이다. exact sea level border, Perlin max
    displacement depth 정도의 shallow ocean-owned below-sea border band, 그리고 그 이상 ocean-owned dry
    terrain은 ordinary land와 같은 micro relief map을 사용한다. 단, Perlin이 enabled이면 sea level 아래
    ocean terrain bed, river terrain bed, river bank/shoulder field에는 별도 bounded Perlin offset을 적용할
    수 있다. bed offset은 water level 계산에 쓰는 기준 bed나 ocean sea-level surface를 움직이지 않아야 한다.
19. 기본 deterministic river bed/bank relief는 water surface solve가 아니다. world-space x/z,
    river strength/flow/roughness/gravel hint만 읽고 terrain bed/shoulder를 bounded offset으로 흔든다.
    river water_y, ocean sea level, lake water level을 직접 바꾸면 회귀다. bank/shoulder offset은 finite
    selected-river distance와 near-bank valley strength gate를 통과해야 하며, broad surrounding valley
    terrain에 post-contour noise를 더하면 회귀다.
