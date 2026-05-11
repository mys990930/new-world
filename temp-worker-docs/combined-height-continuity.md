# combined_macro_height Continuity Investigation

## Scope

이 문서는 heightfield visible ceiling 제거 이후에도 보이는 큰 단차가 `combined_macro_height`
생성 경로에서 실제로 발생할 수 있는지 확인하기 위한 분석 노트다. 이 작업은 코드/스펙 수정 없이
후속 패치 순서를 정하기 위한 문서화 전용이다.

## Loop Notes

- 2026-05-11: Added ignored macro_field neighbor-delta diagnostic. Initial top deltas were mostly
  river carve: seed 7 max `0.133583`, p95 `0.006380`; top pairs had same land kind but
  `river_valley_strength` jumps. Reduced `river_carve_scale` `0.34 -> 0.05`; user rerun showed seed
  7 max down to `0.023989`, with remaining top deltas shifting to owner/site macro elevation jumps.
- 2026-05-11: Patched non-coast `owner_sample` boundary elevation blend to converge to 50/50 at the
  noisy boundary instead of retaining a 65/35 primary bias. This targets same-kind owner switches
  where site `signed_macro_elevation` leaked as a cell-unit jump. Segment bucket indexing was also
  tried; neither materially reduced seed42/91, and a wider boundary radius worsened p95.
- 2026-05-11: User rerun showed river scale `0.05` helped but seed 42/91 top pairs became same-kind
  Continent site-elevation jumps. Boundary-only tweaks had small/negative impact, so macro_field now
  samples land/continent `macro_elevation` from nearby same-kind sites with IDW while leaving owner
  `surface_kind`/masks hard. With IDW radius `1536`, offset `192`, and river scale `0.02`, diagnostic
  max/p95 became seed7 `0.009037/0.001620`, seed42 `0.009361/0.005279`, seed91 `0.005738/0.003381`.
- 2026-05-11: Added dense local diagnostic around each coarse top pair. Before IDW radius taper,
  seed42 still had dense spacing-1 max `0.001393`, p95 `0.000522`, over_1block `510/8320`. After
  tapering IDW weights to zero at radius edge, dense spacing-1 results were seed7
  `0.000238/0.000234/0`, seed42 `0.000203/0.000190/0`, seed91 `0.000295/0.000268/0`
  as max/p95/over_1block. Conclusion: measured top areas no longer show combined-height
  discontinuity at 1-block density; coarse deltas decompose into gradients.

## 1. Actual Calculation Path

`combined_macro_height`는 다음 코드 경로로 생성된다.

1. `src/world/generation/macro_field/mod.rs:273`
   - `generate_macro_field_tile(...)`
   - `MacroFieldRasterContext::new(...)`로 graph/macro/hydrology/boundary lookup context를 만든다.
   - `rasterize_influence_fields(...)`로 ridge/coast/river influence field를 tile sample grid에 미리 rasterize한다.
   - 각 sample index를 `config.sample_position(index)`로 world X/Z 위치로 바꾼다.
   - `sample_macro_field_point_with_influence(...)`를 병렬 호출한다.
2. `src/world/generation/macro_field/mod.rs:571`
   - `rasterize_influence_fields(...)`
   - ridge/coast source는 boundary curve distance field로 rasterize한다.
   - river source는 anti-aliased polyline stroke field로 rasterize한다.
3. `src/world/generation/macro_field/mod.rs:459`
   - `sample_macro_field_point_with_influence(...)`
   - `context.owner_sample(position, config)`에서 owner site, macro elevation, boundary blend hint를 얻는다.
   - owner site의 `surface_kind`로 `ocean_mask`, `lake_mask`, `dry_basin_mask`를 0/1로 만든다.
   - rasterized influence로 `coast_mask`, `ridge_influence`, `river_valley_strength`,
     `river_flow_hint`를 가져온다.
   - `combine_macro_height(...)`에 전달한다.
4. `src/world/generation/macro_field/mod.rs:1115`
   - `MacroFieldRasterContext::owner_sample(...)`
   - nearest site와 nearest noisy boundary side 판정을 결합한다.
   - boundary radius 밖이면 nearest site의 hard `signed_macro_elevation`을 그대로 쓴다.
   - boundary radius 안이면 primary/secondary site를 정하고 제한적 elevation blend를 적용한다.
5. `src/world/generation/macro_field/mod.rs:1441`
   - `combine_macro_height(...)`
   - `macro_elevation + ridge_raise - river_carve`를 만든다.
   - coast/lake flatten을 적용한다.
   - dry basin이면 `dry_basin_height_profile(...)`로 다시 변환한다.
   - 최종 값을 `-2.0..2.0`으로 clamp한다.

heightfield는 이후 이 값을 `src/world/generation/heightfield/mod.rs`의 block scale로 매핑한다.
현재 기본 scale은 normalized `-0.5..0.0..1.0`을 block `-1024..0..2048`로 읽으므로,
`combined_macro_height`의 약 `0.00049` 차이가 visible 1 block에 해당한다.

## 2. Candidate Verification

### A. Owner Site Hard Switch / Insufficient Boundary Blend

- 근거
  - `src/world/generation/macro_field/mod.rs:1096` `nearest_site(...)`
  - `src/world/generation/macro_field/mod.rs:1115` `owner_sample(...)`
  - `src/world/generation/macro_field/mod.rs:1154` `nearest_boundary(...)`
  - `src/world/generation/macro_field/mod.rs:1218` `BoundaryEdgeRef::side_sample(...)`
  - `src/world/generation/macro_field/mod.rs:1245` `BoundarySideSample::primary_site(...)`
  - `src/world/generation/macro_field/mod.rs:1261` `matches_left_side(...)`

- 실제 discontinuity 가능성
  - 높다.
  - owner는 nearest site만 쓰는 것이 아니라 noisy boundary curve side 판정으로 primary site를 고른다.
  - boundary radius 안에서는 elevation blend를 하지만 일반 boundary는
    `primary * (1.0 - blend * 0.35) + secondary * (blend * 0.35)`만 섞는다.
    즉 최대 blend에서도 secondary 반영이 35%라서 양쪽 site elevation 차가 크면 hard switch가 크게 남는다.
  - `surface_kind`는 blended value가 아니라 primary site의 kind를 사용하므로 mask 전환은 더 hard하다.

- 확인 diagnostic/test/dump
  - 인접 sample pair 중 `combined_macro_height` delta가 큰 곳을 찾고,
    각 sample의 `nearest_site`, `surface_kind`, `macro_elevation`,
    `owner_sample.primary`, `coast_boundary_blend`, `dry_basin_rim_blend`,
    nearest boundary id/distance/side를 dump한다.
  - 같은 boundary를 가로지르는 1D transect test를 만들고, sample spacing 1 block에서
    `macro_elevation`과 `combined_macro_height`가 monotonic/continuous에 가까운지 확인한다.
  - 특히 non-coast land/land boundary에서 elevation delta가 큰 pair를 우선 찾는다.

- 우선순위
  - P0.

- 패치 방향 초안
  - boundary 안에서 elevation blend 비중을 stronger/full blend로 바꾸거나, owner elevation은 side hard switch가 아니라
    distance-weighted field로 만들기.
  - surface kind는 계속 categorical로 두더라도 height source는 양쪽 site elevation을 충분히 섞기.
  - non-coast land/land boundary와 coast boundary를 별도 profile로 나누어 테스트 고정.

### B. Ocean/Lake/Dry/Coast Hard Mask 0/1 Transition

- 근거
  - `src/world/generation/macro_field/mod.rs:459` `sample_macro_field_point_with_influence(...)`
  - `src/world/generation/macro_field/mod.rs:465` 이후 `ocean_mask`, `lake_mask`, `dry_basin_mask`
  - `src/world/generation/macro_field/mod.rs:483` `coast_mask`
  - `src/world/generation/macro_map/mod.rs:1043` `surface_kind(...)`

- 실제 discontinuity 가능성
  - 높다.
  - `ocean_mask`, `lake_mask`, `dry_basin_mask`는 continuous mask가 아니라 primary owner site의
    `surface_kind`에서 바로 0.0 또는 1.0이 된다.
  - dry basin은 `combine_macro_height`에서 boolean branch를 타고 `dry_basin_height_profile(...)`로
    다른 함수가 적용된다.
  - coast는 distance influence가 있지만 `site_coastness`와 `owner_sample.coast_boundary_blend * 0.35`
    및 coast source curve influence의 max로 만들어진다. height source 자체와 동일한 continuous owner blend는 아니다.

- 확인 diagnostic/test/dump
  - 큰 delta pair를 surface kind transition별로 bucketize한다:
    land-land, land-ocean, land-lake, land-dry, dry-land, coast-adjacent.
  - 각 pair에서 mask delta와 `combine_macro_height` contribution을 분해해서 dump한다:
    `macro_elevation`, `river_carve`, `flatten`, `flatten_target`, dry basin profile before/after.
  - boundary transect에서 `surface_kind`, mask, `combined_macro_height`를 함께 출력한다.

- 우선순위
  - P0 for ocean/lake/dry transitions.
  - P1 for coast mask alone, because coast influence is already distance based but may still amplify.

- 패치 방향 초안
  - height-relevant masks는 categorical owner와 분리해서 distance/blend based scalar로 만들기.
  - ocean/lake/dry categorical label은 남기되, `combine_macro_height`에는 blended `water_influence`,
    `dry_basin_influence` 같은 continuous scalar를 넣기.
  - dry basin profile도 boolean branch가 아니라 influence mix로 적용하는 방향 검토.

### C. Coast/Lake Flatten Target Pull

- 근거
  - `src/world/generation/macro_field/mod.rs:1441` `combine_macro_height(...)`
  - `coast_flatten = coast_mask * config.coast_flatten_strength`
  - `lake_flatten = lake_mask * config.lake_flatten_strength`
  - `flatten_target = -0.035` for ocean/lake, otherwise `0.0`
  - 기본값은 `DEFAULT_MACRO_FIELD_COAST_FLATTEN_STRENGTH = 0.82`,
    `DEFAULT_MACRO_FIELD_LAKE_FLATTEN_STRENGTH = 0.96`.

- 실제 discontinuity 가능성
  - 중간에서 높음.
  - flatten 자체는 scalar interpolation이라 `coast_mask`가 continuous면 continuous다.
  - 하지만 lake mask는 hard 0/1이고, ocean/lake 여부도 hard branch로 `flatten_target`을 바꾼다.
  - coast flatten은 source elevation이 이미 sea-level 근처가 아닌 경우, coast area 전체를 강하게 0 근처로 끌어당긴다.
    이때 coast influence field가 계단형이거나 owner/mask가 바뀌면 visible jump가 커진다.

- 확인 diagnostic/test/dump
  - `combined_macro_height`를 contribution별로 분해한다:
    `base = macro_elevation + ridge_raise - river_carve`,
    `flatten`, `flatten_target`, `after_flatten`.
  - coast/lake 주변 transect에서 `macro_elevation`이 이미 sea-level aligned인지,
    flatten이 얼마나 보정하고 있는지 측정한다.
  - flatten disabled/config lower 값과 current 값의 delta를 test-only helper나 local diagnostic으로 비교한다.

- 우선순위
  - P1.

- 패치 방향 초안
  - coast flatten을 source-of-truth로 쓰지 말고 보정/guard로 축소한다.
  - coast elevation source가 macro_map 단계에서 이미 sea-level aligned되도록 강화한다.
  - lake/ocean flatten target branch는 continuous water influence 기반으로 바꾼다.

### D. River Carve Strength / Flow Hint Influence

- 근거
  - `src/world/generation/macro_field/mod.rs:571` `rasterize_influence_fields(...)`
  - `src/world/generation/macro_field/mod.rs:695` `rasterize_curve_anti_aliased_polyline_field(...)`
  - `src/world/generation/macro_field/mod.rs:760` `rasterize_segment_anti_aliased_stroke(...)`
  - `src/world/generation/macro_field/mod.rs:1175` `river_valley(...)`
  - `src/world/generation/macro_field/mod.rs:1441` `combine_macro_height(...)`
  - `src/world/generation/macro_field/mod.rs:1607` `river_valley_strength_for_distance(...)`

- 실제 discontinuity 가능성
  - 중간.
  - river field는 anti-aliased stroke라 direct polyline distance보다 smoother를 의도한다.
  - 하지만 valley strength는 여러 segment 중 max로 합성되고, flow hint는 weighted average로 들어간다.
  - `river_carve = river_valley_strength * 0.34 * (0.86 + river_flow_hint * 0.10) * (1.0 - ocean_mask)`라
    normalized scale에서 매우 큰 값을 깎을 수 있다. heightfield block scale로는 작은 strength 차이도 큰 단차가 된다.
  - ocean_mask가 hard 1로 바뀌면 river carve가 갑자기 0이 된다.

- 확인 diagnostic/test/dump
  - 큰 delta pair가 selected river curve radius 안인지 확인한다.
  - `river_valley_strength`, `river_flow_hint`, computed `river_carve`, nearest river distance를 dump한다.
  - connected segment joint와 flow transition 지점에서 transect를 찍어 `river_valley_strength`가 급변하는지 확인한다.

- 우선순위
  - P1 if jumps align with river corridors.
  - P2 otherwise.

- 패치 방향 초안
  - river carve를 heightfield-visible block scale 기준으로 bounded slope/strength로 조정한다.
  - flow hint transitions를 더 부드럽게 만들거나 curve network continuity를 강화한다.
  - ocean/lake mask hard gate 대신 continuous water influence로 river carve attenuation을 적용한다.

### E. Macro Map Site Elevation Cell-Unit Jump

- 근거
  - `src/world/generation/macro_map/mod.rs:106` `MacroSite`
  - `src/world/generation/macro_map/mod.rs:215` `generate_macro_map(...)`
  - `src/world/generation/macro_map/mod.rs:746` `macro_field_sample_from_context(...)`
  - `src/world/generation/macro_map/mod.rs:795` `raw_signed_macro_elevation`
  - `src/world/generation/macro_map/mod.rs:807` `signed_macro_elevation`
  - `src/world/generation/macro_map/mod.rs:898` `coastal_land_elevation_ramp(...)`

- 실제 discontinuity 가능성
  - 높다.
  - macro_map는 site 단위 `signed_macro_elevation`을 만든다.
  - macro_field는 sample마다 owner site를 고르고 그 site value를 읽는다.
  - boundary blend가 없거나 약한 곳에서는 site cell 단위 값이 그대로 field에 들어온다.
  - macro_map 자체는 graph/site discrete field이며, macro_field가 continuous raster field로 만드는 책임을 일부 가진다.

- 확인 diagnostic/test/dump
  - 큰 delta pair의 양쪽 `nearest_site` id와 `signed_macro_elevation` 차이를 출력한다.
  - 같은 ownership class land-land boundary에서 site elevation 차이가 큰 case를 seed scan한다.
  - macro_map edge의 `signed_elevation_gradient`와 macro_field neighbor delta를 correlate한다.

- 우선순위
  - P0, owner hard switch와 같은 root cause로 묶어서 확인.

- 패치 방향 초안
  - macro_field owner elevation sampling을 site hard value에서 boundary-aware blended field로 전환한다.
  - 더 장기적으로는 macro_map site elevation을 corner/edge interpolation 가능한 field source로 제공한다.
  - graph cell ownership과 scalar height field source를 분리한다.

### F. Chamfer/Raster Distance Field Stair-Stepping

- 근거
  - `src/world/generation/macro_field/mod.rs:625` `rasterize_curve_distance_field(...)`
  - `src/world/generation/macro_field/mod.rs:850` `rasterize_segment_sources(...)`
  - `src/world/generation/macro_field/mod.rs:882` `propagate_chamfer_distance(...)`
  - `src/world/generation/macro_field/mod.rs:935` `update_from_neighbor(...)`
  - `src/world/generation/macro_field/mod.rs:695` river anti-aliased path is separate.

- 실제 discontinuity 가능성
  - 중간.
  - coast/ridge distance field는 curve를 source pixels로 찍은 뒤 chamfer propagation한다.
  - 이 값은 grid spacing에 묶인 계단형 근사 distance가 될 수 있다.
  - coast influence가 flatten에 강하게 들어가므로, coast distance stair step이 visible height stair step으로 증폭될 수 있다.
  - ridge는 현재 `DEFAULT_MACRO_FIELD_RIDGE_HEIGHT_SCALE = 0.0`이라 combined height에는 직접 영향이 없다.

- 확인 diagnostic/test/dump
  - coast source 근처에서 exact polyline distance와 rasterized `coast_distance_blocks`를 비교한다.
  - sample spacing 1, 2, 4, 16, 32에서 같은 transect의 coast influence stair amplitude를 비교한다.
  - coast flatten contribution이 stair pattern과 같은 위치에서 변하는지 확인한다.

- 우선순위
  - P2.

- 패치 방향 초안
  - coast distance는 raster chamfer 대신 exact polyline distance query 또는 signed distance cache를 사용한다.
  - 최소한 influence를 sample-local exact distance로 recompute하는 diagnostic path를 만들어 비교한다.
  - ridge는 height scale이 활성화될 때 같은 문제를 다시 점검한다.

## 3. Coast Flatten 판단

사용자 질문:

> coast는 굳이 flatten 안하더라도, y = 0이 해수면 판정이면 coast height도 그 근처에서 놀아야하는 것 아니냐?

코드 기준 답:

- 현재 macro_map 단계에는 이미 coast elevation을 sea-level 근처로 맞추려는 장치가 있다.
  `src/world/generation/macro_map/mod.rs:807`에서 land-owned site는
  `coastal_land_elevation_ramp(...)`를 거친다.
- 따라서 이상적인 방향은 coast의 source elevation 자체가 sea-level aligned인 것이다.
  즉 coast flatten은 primary source-of-truth라기보다, macro_map/site ownership과 boundary/rasterization에서
  생기는 잔여 오차를 누르는 보정 또는 guard에 가깝게 보는 편이 맞다.
- 현재 `combine_macro_height(...)`의 coast flatten은 강하다. `coast_mask * 0.82`로 base height를
  `0.0` 쪽으로 당긴다. 이 말은 source elevation이 이미 안정적이라면 flatten은 작아도 되어야 하고,
  source elevation이 불연속이라면 flatten은 문제를 해결하기보다 coast influence stair나 mask 전환을
  visible height로 옮기는 역할도 할 수 있다.
- ocean/lake는 별도다. water 판정은 heightfield에서 `y = 0` visible water surface 정책을 갖고 있지만,
  macro_field `combined_macro_height`에서는 ocean/lake `flatten_target = -0.035`로 얕은 수면 아래 값을 만든다.
  이 값 자체가 visible water top의 source-of-truth는 아니다.

판단:

- coast height는 가능하면 macro_map/macro_field source 단계에서 이미 sea-level aligned여야 한다.
- coast flatten은 source-of-truth가 아니라 보정이어야 한다.
- 후속 패치에서는 먼저 coast source elevation과 owner boundary blend가 충분한지 확인하고,
  그 다음 coast flatten strength/target을 줄이거나 continuous influence 기반으로 바꾸는 순서가 좋다.

## 4. Recommended Processing Order

1. P0: 큰 neighbor delta를 찾는 diagnostic/test를 먼저 만든다.
   - pair별 `combined_macro_height` delta, owner site ids, surface kind, macro elevation, masks,
     river carve, flatten contribution을 분해해 원인 bucket을 만든다.
2. P0: owner site hard switch와 macro_map site elevation cell-unit jump를 같이 처리한다.
   - 이 둘은 같은 root cause일 가능성이 높다.
3. P0: ocean/lake/dry hard mask transition을 처리한다.
   - water/dry categorical label과 height influence를 분리하는 방향.
4. P1: coast flatten target pull을 확인하고 조정한다.
   - source elevation sea-level alignment가 충분하면 flatten은 약화한다.
5. P1/P2: river carve를 jump 위치가 river와 겹칠 때 처리한다.
   - river-aligned jump가 확인될 때 우선순위를 올린다.
6. P2: chamfer/raster distance field stair-stepping을 처리한다.
   - coast influence stair가 실제 height stair와 일치할 때 exact distance or better SDF로 교체한다.

## 5. Why This Is Probably Not Heightfield-Only

heightfield 쪽 일반 terrain 경로는 이제 raw block height를 floor integer snap해서 visible surface로 내보낸다.
따라서 1 block 초과 jump가 보이면 일반 land에서는 `combined_macro_height` 또는 그 전 source field의
delta가 이미 block scale에서 1 이상이라는 뜻이다.

heightfield에서 아직 예외로 남는 부분은 ocean/lake visible water `y = 0`, shoreline standing-water
ceiling, river water descent다. jump가 이 water/shoreline/river-water 예외와 겹치지 않는 일반 land
구간이라면 macro_field/macro_map source continuity가 우선 의심 대상이다.

## 6. Green Transition Geometry Correction

- 색상 ramp 문제가 아니라 실제 heightfield geometry step으로 재분석했다.
- seed42 green transition coarse scan(1536 block footprint, 32 columns, spacing 48)은 Land/Land, river/coast 없음에서도 `surface_delta=18`, `raw_delta=17.736`, `combined_delta=0.008660`을 만들었다.
- 같은 top pair 주변을 1-block dense window로 재측정하면 `max_surface_delta=1`, `max_raw_delta=0.381`, `max_combined_delta=0.000186`, `over_one=0`이었다.
- 결론: 측정된 green transition top은 macro source 불연속이 아니라 free-window coarse sample spacing이 연속 경사를 큰 column-to-column geometry step으로 보이게 만든 사례다.
- 패치: heightfield preview free-window 기본을 1024 block footprint / 1024 X columns로 바꿔 기본 preview도 1 world block당 1 column density를 사용한다. 더 넓은 overview가 필요하면 `--world-span-blocks`/`--columns-x`를 명시한다.
