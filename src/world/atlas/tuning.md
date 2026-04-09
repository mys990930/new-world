# tuning

## 역할

- atlas prototype의 조절 가능한 기본값을 한 곳에 모은다.
- generation, resolver, biome preview debug가 어떤 수치를 기준으로 동작하는지 한눈에 보이게 한다.

현재 기본값 구현 위치는 `tuning.rs`의 `AtlasTuning::default()`다.

---

## 읽는 법

이 파일은 각 필드를 "무슨 뜻인지"와 "올리거나 내리면 뭐가 바뀌는지" 기준으로 읽는다.

이름 규칙은 먼저 아래처럼 이해하면 된다.

| 패턴 | 뜻 | 보통 값을 올리면 | 보통 값을 내리면 |
| --- | --- | --- | --- |
| `*_scale` | 월드 공간에서 패턴이 바뀌는 길이 | 더 큰 덩어리, 더 완만한 변화 | 더 촘촘한 변화, 더 자잘한 패턴 |
| `*_octaves` | 노이즈 층 수 | 더 복합적인 패턴 | 더 단순한 패턴 |
| `*_lacunarity` | 옥타브마다 주파수가 얼마나 빨리 올라가는지 | 세부가 더 빨리 잘게 쪼개짐 | 세부가 더 완만해짐 |
| `*_gain` | 높은 옥타브의 영향력 | 잔무늬/고주파 영향 증가 | 큰 흐름 위주로 단순화 |
| `*_weight` | 해당 요인이 최종 값에 미치는 비중 | 그 요인이 더 강하게 반영됨 | 그 요인이 덜 반영됨 |
| `*_threshold` | 어떤 분류/판정이 시작되는 경계 | 그 현상이 더 드물어짐 | 그 현상이 더 흔해짐 |
| `*_bonus`, `*_bias` | 기본값에 더해지는 상수 | 해당 성향을 전반적으로 밀어줌 | 해당 성향을 약하게 만듦 |
| `*_penalty` | 특정 성향을 깎는 상수 | 해당 성향을 더 강하게 억제 | 해당 성향을 덜 억제 |
| `*_min`, `*_max` | smoothstep 구간 | 시작/완료 지점이 이동 | 시작/완료 지점이 반대로 이동 |
| `*_light`, `*_dark` | biome preview 색상의 밝은/어두운 끝색 | 색 팔레트 자체가 바뀜 | 색 팔레트 자체가 바뀜 |
| `*_shade_strength` | 높이/깊이에 따른 명암 대비 | 같은 biome 안에서도 진하기 차이가 커짐 | 같은 biome 안에서도 평평한 색으로 보임 |

---

## Quick Knobs

가장 자주 손댈 가능성이 큰 필드는 이쪽이다.

| 필드 | 지금 하는 일 | 올리면 | 내리면 |
| --- | --- | --- | --- |
| `normalization.land_threshold` | landness를 육지로 인정하는 기준 | 육지가 줄고 바다가 늘어남 | 육지가 늘고 바다가 줄어듦 |
| `continent.primary_scale` | 대륙 대형 덩어리 크기 | 더 큰 대륙/해양 | 더 자잘한 대륙/군도 |
| `continent.secondary_scale` | 대륙 가장자리/부대륙 스케일 | 더 큰 반도/부속 대륙 | 더 잘게 찢긴 해안 |
| `continent.coast_penalty` | coast roughness가 landness를 깎는 정도 | 해안이 더 많이 부서짐 | 해안이 더 둥글고 안정적 |
| `ridge.primary_scale` | 큰 산맥/능선 스케일 | 긴 산맥과 넓은 산악권 | 잘게 끊긴 산지 |
| `terrain.macro_mountain_weight` | 산이 macro elevation을 얼마나 밀어올리는지 | 산이 더 높고 존재감 커짐 | 산이 낮고 평탄해짐 |
| `hydrology.river_threshold` | 강으로 보이기 시작하는 flow 기준 | 큰 강만 남음 | 자잘한 강줄기까지 늘어남 |
| `climate.temperature_field_scale` | 대온도대의 크기 | 큰 기후 벨트 | 지역마다 온도 변화가 빨라짐 |
| `climate.humidity_field_scale` | 대습도대의 크기 | 큰 습윤/건조 벨트 | 습도 패턴이 더 잘게 변함 |
| `resolver.forest_threshold` | 숲 biome preview로 읽는 기준 | 숲이 줄고 초원/스텝이 늘어남 | 숲이 쉽게 생김 |

---

## normalization

| 필드 | 뜻 | 올리면 | 내리면 |
| --- | --- | --- | --- |
| `land_threshold` | `landness`가 이 값 이상이면 육지로 본다 | 육지 면적 감소 | 육지 면적 증가 |
| `ocean_distance_normalizer` | 바다까지의 거리값을 0..1로 누르는 스케일 | 내륙성 증가가 더 천천히 쌓임 | 조금만 안쪽으로 들어가도 내륙성이 빨리 커짐 |
| `coast_distance_normalizer` | 해안 거리 정규화 스케일 | coast 계열이 더 넓고 완만하게 퍼짐 | coast 영향이 해안선 근처에만 집중 |
| `continent_core_normalizer` | 대륙 중심부로 인정되는 거리 스케일 | continent core가 더 천천히 커짐 | 조금만 깊어도 core로 읽힘 |
| `coast_factor_distance` | coast factor가 약해지는 실제 거리 | 해안 영향이 더 깊게 들어옴 | 해안 영향이 얕게 끝남 |
| `river_distance_normalizer` | 강/호수로부터의 거리 정규화 스케일 | riverine 영향이 더 넓게 퍼짐 | 강 근처에서만 riverine이 강함 |
| `ridge_landness_min` | ridge가 landness에 반응하기 시작하는 하한 | 능선이 더 내륙 위주로 제한 | 해안 가까이에서도 ridge가 더 살아남음 |
| `ridge_landness_max` | ridge가 landness에 완전히 실리는 상한 | 강한 ridge가 더 깊은 육지 쪽에서만 형성 | 해안 가까이도 ridge가 빨리 강해짐 |

---

## continent

| 필드 | 뜻 | 올리면 | 내리면 |
| --- | --- | --- | --- |
| `warp_scale` | 대륙 마스크를 휘게 만드는 warp 노이즈의 파장 | 더 큰 단위로 휘어짐 | 더 자잘하게 비틀림 |
| `warp_amplitude` | 대륙 외곽을 얼마나 많이 비트는지 | 해안과 대륙 경계가 더 요동침 | 더 둥글고 매끈한 대륙 |
| `primary_scale` | 가장 큰 대륙 덩어리 스케일 | 대륙과 해양이 더 거대해짐 | 대륙이 더 잘게 쪼개짐 |
| `primary_octaves` | 대륙 대형 노이즈의 복잡도 | 큰 흐름 안에 복합적인 굴곡 증가 | 더 단순한 큰 덩어리 |
| `primary_lacunarity` | 대형 대륙 노이즈의 세부 증가 속도 | 큰 패턴 위에 잘게 흔들리는 가장자리 증가 | 패턴이 더 둔함 |
| `primary_gain` | 대형 대륙 노이즈에서 고주파 기여도 | 큰 대륙 안의 잔 굴곡 증가 | 대륙 윤곽이 더 단순 |
| `secondary_scale` | 부대륙/반도/해안 굴곡 스케일 | 더 큰 반도와 하위 대륙 | 더 잘게 찢긴 해안선 |
| `secondary_octaves` | secondary 노이즈 복잡도 | 반도와 해안 세부 증가 | 부속 지형이 단순 |
| `secondary_lacunarity` | secondary 세부 증가 속도 | 잘게 꺾이는 해안 증가 | 해안선이 덜 복잡 |
| `secondary_gain` | secondary 고주파 영향 | 해안 세부 영향 증가 | 큰 형태 위주 |
| `island_scale` | 군도/섬 노이즈 스케일 | 더 큰 섬 덩어리 | 자잘한 섬 분포 |
| `island_octaves` | 섬 노이즈 복잡도 | 군도 패턴이 더 복합 | 섬 분포가 단순 |
| `island_lacunarity` | 섬 세부 증가 속도 | 잔섬/톱니 모양 증가 | 둥근 섬 위주 |
| `island_gain` | 섬 노이즈 고주파 영향 | 군도 디테일 증가 | 군도 영향 약화 |
| `coast_scale` | coast roughness 패턴의 크기 | 더 큰 해안 굴곡 | 더 촘촘한 톱니형 해안 |
| `coast_octaves` | coast roughness 복잡도 | 해안 가장자리가 더 복합 | 해안이 단순 |
| `coast_lacunarity` | coast roughness 세부 증가 속도 | 잘게 깨지는 해안 증가 | 더 완만한 해안 |
| `coast_gain` | coast roughness 고주파 영향 | 해안 노이즈가 더 거칠게 반영 | 거칠기 완화 |
| `primary_weight` | primary continent 노이즈 비중 | 대형 대륙 흐름이 강해짐 | secondary/island 영향이 상대적으로 커짐 |
| `secondary_weight` | secondary continent 노이즈 비중 | 부대륙/반도/해안 굴곡 증가 | 대형 흐름 위주 |
| `island_weight` | island 노이즈 비중 | 군도와 섬이 늘어남 | 섬이 줄고 큰 육지 위주 |
| `coast_penalty` | coast roughness가 landness를 깎는 정도 | 해안 절단, 만, 반도 증가 | 둥글고 연결된 대륙 |
| `bias` | landness 전체 bias | 월드 전체가 더 육지 쪽으로 이동 | 월드 전체가 더 바다 쪽으로 이동 |

---

## ridge

| 필드 | 뜻 | 올리면 | 내리면 |
| --- | --- | --- | --- |
| `warp_scale` | ridge field를 비트는 warp 파장 | 큰 단위로 휘는 산맥 | 자잘하게 흔들리는 산맥 |
| `warp_amplitude` | 산맥이 얼마나 휘고 꺾이는지 | 더 구불구불한 산맥 | 더 곧고 단순한 능선 |
| `primary_scale` | 주 산맥 스케일 | 긴 산맥/넓은 산악권 | 짧고 잘린 산지 |
| `primary_octaves` | 주 ridge 복잡도 | 산맥의 가지와 변화 증가 | 산맥 패턴 단순화 |
| `primary_lacunarity` | 주 ridge 세부 증가 속도 | 산맥의 잔 굴곡 증가 | 더 완만 |
| `primary_gain` | 주 ridge의 고주파 비중 | 더 험한 능선 | 부드러운 능선 |
| `secondary_scale` | 보조 ridge 스케일 | 큰 보조 산줄기 | 자잘한 측면 산줄기 |
| `secondary_octaves` | 보조 ridge 복잡도 | 산줄기 파생 증가 | 덜 복합 |
| `secondary_lacunarity` | 보조 ridge 세부 증가 속도 | 잔 능선 증가 | 완만 |
| `secondary_gain` | 보조 ridge 고주파 비중 | 측면 디테일 증가 | 보조 ridge 영향 약화 |
| `primary_weight` | 주 ridge 비중 | 큰 산맥 축이 뚜렷해짐 | 보조 ridge 영향 상대 증가 |
| `secondary_weight` | 보조 ridge 비중 | 갈래 산줄기 증가 | 주 ridge 위주로 단순화 |

---

## terrain

| 필드 | 뜻 | 올리면 | 내리면 |
| --- | --- | --- | --- |
| `macro_base_height` | 육지 elevation의 기본 바닥값 | 전체 육지가 약간 높아짐 | 저지대가 많아짐 |
| `macro_landness_weight` | landness가 고도에 주는 영향 | 육지일수록 더 높아짐 | landness와 elevation 연동 약화 |
| `macro_continent_core_weight` | 대륙 중심부가 높아지는 비중 | 내륙 고원/중심부 상승 | 중심부와 해안 차이 축소 |
| `macro_mountain_weight` | mountain mass가 elevation에 주는 영향 | 산지가 더 높아짐 | 산이 낮고 둔해짐 |
| `macro_coast_penalty` | coast factor가 elevation을 깎는 정도 | 해안 저지대가 많아짐 | 해안도 쉽게 높아짐 |
| `macro_ocean_floor_scale` | ocean cell의 sea-floor 표현 강도 | 바다 깊이 gradient가 커짐 | 바다 깊이 표현이 평평해짐 |
| `detail_scale` | ruggedness용 미세 detail 스케일 | 더 큰 단위의 거친 지형 | 더 자잘한 디테일 |
| `detail_octaves` | detail 복잡도 | 미세 표면 변화 증가 | 단순화 |
| `detail_lacunarity` | detail 세부 증가 속도 | 잔 굴곡 증가 | 완만 |
| `detail_gain` | detail 고주파 비중 | ruggedness 노이즈 강화 | detail 약화 |
| `slope_scale` | neighbor height diff를 slope로 바꾸는 증폭값 | slope가 전반적으로 커짐 | slope가 둔화됨 |
| `rugged_ridge_weight` | ridge가 ruggedness에 주는 비중 | 능선 주변이 더 험해짐 | ridge와 ruggedness 결속 약화 |
| `rugged_slope_weight` | slope가 ruggedness에 주는 비중 | 급경사 지역이 더 험하게 읽힘 | slope 영향 약화 |
| `rugged_detail_weight` | detail 노이즈가 ruggedness에 주는 비중 | 자잘한 거침이 늘어남 | 큰 흐름 위주 |
| `basin_offset` | basinness 계산 전 주는 상수 보정 | 분지로 읽히는 영역이 늘어남 | basin 판정이 더 엄격해짐 |
| `basin_scale` | basinness 증폭값 | 분지/저지대 contrast 증가 | basinness가 평평해짐 |
| `pass_ridge_penalty` | ridge가 pass potential을 깎는 정도 | 험한 산맥 통과가 더 어려워짐 | 능선 사이 고개가 늘어남 |
| `pass_slope_penalty` | slope가 pass potential을 깎는 정도 | 급경사 통로가 줄어듦 | 경사 큰 곳도 pass로 남음 |
| `pass_mean_diff_weight` | 주변 평균과의 차이가 pass에 주는 영향 | 주변보다 살짝 낮은 고개가 더 잘 드러남 | 고개 판정이 약해짐 |
| `mountain_cluster_scale` | mountain cluster 스케일 | 큰 산악 덩어리 | 잘게 나뉜 산지 |
| `mountain_cluster_octaves` | mountain cluster 복잡도 | 산악권 내부 변화 증가 | 단순한 산지 |
| `mountain_cluster_lacunarity` | mountain cluster 세부 증가 속도 | 산지 디테일 증가 | 완만 |
| `mountain_cluster_gain` | mountain cluster 고주파 비중 | 산지 덩어리 안의 자잘한 변화 증가 | 큰 산지 위주 |
| `mountain_ridge_weight` | ridge가 mountain mass에 주는 비중 | 선형 산맥 중심 구조 강화 | cluster형 산지 영향 증가 |
| `mountain_cluster_weight` | cluster가 mountain mass에 주는 비중 | 덩어리 산악권 증가 | ridge 축 중심 산지 |
| `mountain_base_min` | mountain mass가 올라가기 시작하는 하한 | mountain 판정이 더 늦게 시작 | mountain이 더 쉽게 생김 |
| `mountain_base_max` | mountain mass가 충분히 높아지는 상한 | 강한 mountain core가 더 드물어짐 | 강한 mountain core가 쉽게 생김 |
| `mountain_continent_min` | 대륙성 보정이 산을 허용하기 시작하는 하한 | 해안 가까운 산이 줄어듦 | 해안 산지 증가 |
| `mountain_continent_max` | 대륙성 보정이 충분히 실리는 상한 | 내륙 깊은 곳에서만 산이 강해짐 | 조금만 안쪽이어도 산이 강해짐 |
| `mountain_landness_bias` | landness가 continent gating에 더해지는 양 | 육지면 산이 더 쉽게 생김 | continent core가 더 중요해짐 |

---

## climate

| 필드 | 뜻 | 올리면 | 내리면 |
| --- | --- | --- | --- |
| `temperature_field_scale` | 대온도대 스케일 | 큰 온도 벨트, 완만한 변화 | 잘게 바뀌는 온도 패턴 |
| `temperature_octaves` | 온도장 복잡도 | 기온장 내부 변화 증가 | 단순한 온도장 |
| `temperature_lacunarity` | 온도 세부 증가 속도 | 잔 온도 변화 증가 | 완만 |
| `temperature_gain` | 온도 고주파 비중 | 세부 온도 노이즈 강화 | 큰 벨트 위주 |
| `humidity_field_scale` | 대습도대 스케일 | 큰 건습 벨트 | 더 자잘한 습도 패턴 |
| `humidity_octaves` | 습도장 복잡도 | 습도장 변화 증가 | 단순 |
| `humidity_lacunarity` | 습도 세부 증가 속도 | 잔 습도 차이 증가 | 완만 |
| `humidity_gain` | 습도 고주파 비중 | 지역 단위 습도 차이 강화 | 넓은 패턴 위주 |
| `equator_falloff_scale` | 위도 비슷한 온도대가 유지되는 범위 | 위도에 따른 온도 변화가 더 느림 | 위도만 바뀌어도 기온이 빨리 변함 |
| `temperature_equator_weight` | 위도/적도 효과 비중 | 위도 기반 climate belt 강화 | noise와 고도 영향 상대 증가 |
| `temperature_noise_weight` | noise 기반 기온 비중 | 지역별 변칙 온도 증가 | 위도/고도 질서 강화 |
| `temperature_elevation_cooling` | 고도가 temperature를 내리는 정도 | 산이 더 쉽게 추워짐 | 고산 냉대가 약해짐 |
| `temperature_mountain_cooling` | mountain mass가 temperature를 추가로 내리는 정도 | 산악권 냉각 강화 | mountain mass의 냉각 영향 약화 |
| `polar_edge_warm` | polar factor가 시작되는 따뜻한 경계 | 극지 판정이 더 늦게 시작 | 극지 성향이 쉽게 나타남 |
| `polar_edge_cold` | polar factor가 충분히 강해지는 차가운 경계 | 아주 차가운 셀에서만 극지 core | 조금만 차가워도 극지 성향 강화 |
| `humidity_noise_weight` | noise 기반 습도 비중 | 지역별 건습 차이 증가 | 해양/분지/내륙성 영향이 더 지배적 |
| `humidity_ocean_bonus` | 바다 근접 보너스 | 해안이 더 습해짐 | 해안 습윤 효과 약화 |
| `humidity_basin_bonus` | basinness 습도 보너스 | 저지대/분지가 더 습해짐 | basin 영향 약화 |
| `humidity_inland_penalty` | inlandness 건조 패널티 | 내륙이 더 빨리 말라감 | 내륙도 습도 유지 |
| `humidity_rain_shadow_penalty` | 산 + 내륙 조합의 건조화 정도 | 산 너머 내륙이 더 말라감 | rain shadow 약화 |
| `humidity_river_bonus` | riverine이 humidity를 올리는 양 | 강 주변이 더 촉촉해짐 | riverine 습윤 완화 |
| `humidity_lake_bonus` | lake가 humidity를 올리는 양 | 호수 주변 습윤 강화 | lake 주변 차이 약화 |

---

## hydrology

| 필드 | 뜻 | 올리면 | 내리면 |
| --- | --- | --- | --- |
| `river_source_mountain_weight` | 산지가 river source potential에 주는 비중 | 상류 산지 발원 강화 | 산 영향 약화 |
| `river_source_humidity_weight` | 습도가 river source potential에 주는 비중 | 습윤 지역 발원 강화 | 습도 영향 약화 |
| `river_source_slope_weight` | 경사가 river source potential에 주는 비중 | 경사 큰 곳 발원 강화 | slope 영향 약화 |
| `flow_base` | 기본 유량 바닥값 | 전체 river network가 쉬워짐 | 큰 강만 남기 쉬움 |
| `flow_humidity_weight` | 습도가 flow에 주는 비중 | 습윤 지역 하천 밀도 증가 | 습도 영향 약화 |
| `flow_mountain_weight` | 산지가 flow에 주는 비중 | 산악권 유량 강화 | 산 영향 약화 |
| `flow_cold_weight` | 차가운 지역 보정 | 차가운 지역 수계 유지 강화 | 온도 영향 약화 |
| `river_threshold` | 강으로 표시되기 위한 flow 하한 | 굵은 강만 남음 | 자잘한 하천 증가 |
| `river_source_threshold` | 발원 조건을 만족해야 하는 강 판정선 | 발원 조건이 엄격해짐 | 강이 더 쉽게 생김 |
| `river_override_threshold` | 발원 미흡해도 강으로 남는 고유량 예외선 | 아주 큰 flow만 예외 허용 | 발원 약해도 강이 살아남음 |
| `river_min_macro_elevation` | 강으로 인정되는 최소 고도 | 저지대 강이 줄어듦 | 평지/범람원 강도 늘어남 |
| `lake_basin_weight` | basinness가 lake potential에 주는 비중 | 분지 호수 증가 | basin 영향 약화 |
| `lake_flow_weight` | flow가 lake potential에 주는 비중 | 수계 기반 호수 증가 | flow 영향 약화 |
| `lake_sink_bonus` | downhill target이 없는 sink 보너스 | 폐쇄 분지 호수 증가 | sink 효과 약화 |
| `lake_flat_bonus` | 평탄지 보너스 | 평지 호수 증가 | slope 조건이 더 중요해짐 |
| `lake_threshold` | 호수로 보이는 기준 | 큰/분명한 호수만 남음 | 호수 후보 증가 |
| `lake_min_macro_elevation` | lake 허용 최소 고도 | 저지대 호수 감소 | 바다 가까운 저지대 호수 증가 |
| `lake_max_slope` | lake 허용 최대 경사 | 평탄한 곳에만 호수 남음 | 경사진 곳도 호수 가능 |
| `riverine_distance_weight` | 강까지의 거리가 riverine에 미치는 비중 | 강 주변 띠가 넓어짐 | riverine이 더 강줄기 근처에 붙음 |
| `riverine_flow_weight` | flow가 riverine에 미치는 비중 | 큰 강이 더 강하게 드러남 | 거리 중심 riverine |
| `wetness_humidity_weight` | humidity가 wetness에 주는 비중 | 습한 지역 전반이 wet해짐 | 수계/분지 영향 상대 증가 |
| `wetness_river_weight` | riverine이 wetness에 주는 비중 | 강가 습윤 강화 | 강 주변 wetness 약화 |
| `wetness_lake_weight` | lake가 wetness에 주는 비중 | 호수 주변 저습지 강화 | 호수 영향 약화 |
| `wetness_basin_weight` | basinness가 wetness에 주는 비중 | 저지대/분지 습윤 강화 | basin 영향 약화 |
| `aridity_dryness_weight` | 낮은 humidity가 aridity로 번역되는 정도 | 건조대가 더 쉽게 형성 | aridity가 약해짐 |
| `aridity_inland_weight` | inlandness의 건조화 정도 | 내륙 사막/스텝 강화 | 내륙 건조화 약화 |
| `aridity_low_wetness_weight` | wetness 부족이 aridity에 주는 비중 | 저습 지역 건조감 강화 | wetness와 aridity 결속 약화 |
| `aridity_river_relief` | riverine이 aridity를 깎는 정도 | 강이 사막/건조대를 더 잘 완화 | river relief 약화 |
| `alpine_elevation_min` | alpine이 고도에서 시작되는 하한 | alpine이 더 늦게 생김 | alpine이 더 쉽게 생김 |
| `alpine_elevation_max` | 고도만으로 alpine이 충분해지는 상한 | 아주 높은 곳에서만 alpine core | 비교적 낮아도 alpine core 가능 |
| `alpine_elevation_weight` | 고도 기반 alpine 비중 | 높은 산 정상 영향 강화 | mountain mass 비중 상대 증가 |
| `alpine_mountain_min` | mountain mass로 alpine이 시작되는 하한 | alpine이 더 드물어짐 | alpine이 더 쉽게 생김 |
| `alpine_mountain_max` | mountain mass가 alpine core가 되는 상한 | 아주 강한 산악권만 alpine core | alpine core가 쉬워짐 |
| `alpine_mountain_weight` | mountain mass 기반 alpine 비중 | mountain form이 alpine에 더 중요 | pure elevation 비중 상대 증가 |
| `alpine_polar_penalty` | polar가 alpine을 깎는 정도 | 극지와 alpine이 더 분리됨 | polar와 alpine이 겹치기 쉬움 |
| `wetland_wetness_weight` | wetness가 wetland에 주는 비중 | 습한 지역 전체에 습지 증가 | basin/river 중심 습지 |
| `wetland_basin_weight` | basinness의 습지 비중 | 분지형 습지 증가 | basin 영향 약화 |
| `wetland_river_weight` | riverine의 습지 비중 | 강변 습지 증가 | 강변 습지 약화 |
| `wetland_slope_penalty` | slope가 습지를 깎는 정도 | 경사진 곳 습지 억제 강화 | slope 있어도 습지 유지 |

---

## weights

### thermal / moisture axis

| 필드 | 뜻 | 올리면 | 내리면 |
| --- | --- | --- | --- |
| `thermal_centers` | `polar, cold, temperate, warm, hot`가 temperature 축 어디에 중심을 두는지 | 해당 class가 더 높은 temperature 쪽으로 밀림 | 더 낮은 temperature에서도 그 class가 나옴 |
| `thermal_widths` | 각 thermal class의 blending 폭 | 전이대가 넓어지고 dominant가 덜 날카로움 | thermal 경계가 더 날카로움 |
| `moisture_signal_humidity_weight` | humidity가 moisture signal에 주는 비중 | 습도장이 moisture class를 더 지배 | wetness/aridity 영향 상대 증가 |
| `moisture_signal_wetness_weight` | wetness 비중 | 강/호수/분지 근처가 더 wet class로 이동 | humidity 중심 해석 |
| `moisture_signal_aridity_penalty` | aridity가 moisture signal을 깎는 정도 | arid/semi-arid 쪽으로 더 쉽게 감 | 건조 패널티 약화 |
| `moisture_signal_bias` | moisture signal 전체 bias | 전체 월드가 더 습윤 쪽으로 이동 | 더 건조 쪽으로 이동 |
| `moisture_centers` | `arid, semi_arid, subhumid, humid, wet` 중심값 | 해당 class가 더 높은 signal에서 등장 | 더 낮은 signal에서도 등장 |
| `moisture_widths` | 각 moisture class blend 폭 | 건습 전이대가 넓어짐 | 건습 경계가 날카로워짐 |
| `moisture_arid_boost` | aridity가 arid class를 강제로 밀어주는 정도 | 건조 biome preview가 늘어남 | arid class가 약해짐 |
| `moisture_wet_boost` | wetness가 wet class를 밀어주는 정도 | 습지/강가/저지대 wet class 강화 | wet class가 약해짐 |

### terrain form

| 필드 | 뜻 | 올리면 | 내리면 |
| --- | --- | --- | --- |
| `form_mountain_min` | mountain form 시작 하한 | mountain이 더 늦게 시작 | mountain이 더 쉽게 생김 |
| `form_mountain_max` | mountain form이 충분해지는 상한 | 진짜 mountain core가 더 드묾 | strong mountain이 쉬워짐 |
| `form_mountain_mass_weight` | mountain mass가 form mountain에 주는 비중 | 구조적 산악권 강조 | ruggedness 영향 상대 증가 |
| `form_mountain_ruggedness_weight` | ruggedness 비중 | 험한 구릉도 mountain처럼 읽히기 쉬움 | structure 중심 mountain |
| `form_hill_min` | hill form 시작 하한 | hill이 더 드묾 | hill이 쉽게 생김 |
| `form_hill_max` | hill이 충분해지는 상한 | hill core가 더 드묾 | hill dominant가 쉬워짐 |
| `form_hill_macro_elevation_weight` | macro elevation이 hill에 주는 비중 | 높기만 해도 hill로 읽히기 쉬움 | ruggedness 중심 hill |
| `form_hill_mountain_suppression` | mountain이 hill을 누르는 정도 | mountain/hill 분리가 강해짐 | mountain과 hill이 겹쳐 보임 |
| `form_plain_mountain_penalty` | plain에서 mountain을 빼는 강도 | mountain이 plain을 더 강하게 밀어냄 | plain 잔존 증가 |
| `form_plain_hill_penalty` | plain에서 hill을 빼는 강도 | hill이 plain을 더 강하게 밀어냄 | plain 잔존 증가 |

### cover

| 필드 | 뜻 | 올리면 | 내리면 |
| --- | --- | --- | --- |
| `cover_canopy_humidity_weight` | humidity가 canopy에 주는 비중 | 습한 지역이 더 쉽게 숲으로 감 | 습도 영향 약화 |
| `cover_canopy_wetness_weight` | wetness 비중 | 강가/저지대 canopy 증가 | wetness 영향 약화 |
| `cover_canopy_temperate_weight` | temperate/warm climate가 canopy를 밀어주는 기본 비중 | 온대/난대 숲 증가 | 기후 영향 약화 |
| `cover_canopy_warm_bonus_weight` | `warm` 가중치를 temperate와 섞는 비율 | warm 지역 canopy 증가 | temperate 중심 canopy |
| `cover_canopy_aridity_penalty` | aridity가 canopy를 깎는 정도 | 건조 지역 숲 감소 | 건조해도 canopy 유지 |
| `cover_canopy_alpine_penalty` | alpine이 canopy를 깎는 정도 | 고산권 식생 감소 | alpine에서도 canopy 유지 |
| `cover_forest_canopy_weight` | canopy가 forest potential에 주는 비중 | canopy가 곧 숲으로 이어짐 | river/humid 보조 영향 증가 |
| `cover_forest_river_weight` | riverine이 forest를 미는 비중 | 강변 숲 증가 | 강 영향 약화 |
| `cover_forest_humid_weight` | humid class가 forest를 미는 비중 | 습윤 숲 증가 | humidity 영향 약화 |
| `cover_openness_base` | openness 기본값 | 전체적으로 탁 트인 땅 증가 | 전체적으로 닫힌 피복 증가 |
| `cover_openness_aridity_weight` | aridity가 openness에 주는 비중 | 건조 지역 개활지 증가 | aridity 영향 약화 |
| `cover_openness_plain_weight` | plain form이 openness에 주는 비중 | 평원이 더 쉽게 열림 | 평지라도 숲 유지 가능 |
| `cover_openness_canopy_penalty` | canopy가 openness를 깎는 정도 | 숲과 개활지 분리가 선명 | canopy와 openness 공존 쉬움 |
| `cover_openness_wet_penalty` | wet moisture가 openness를 깎는 정도 | wet 지역이 더 닫힘 | wet해도 개방도 유지 |
| `cover_grass_openness_weight` | openness가 grass를 미는 비중 | 열린 초원 증가 | 초원보다 숲/덤불 증가 |
| `cover_grass_subhumid_weight` | subhumid가 grass에 주는 비중 | 적당히 촉촉한 초원 증가 | subhumid 영향 약화 |
| `cover_grass_humid_weight` | humid가 grass에 주는 비중 | humid grassland 증가 | humid는 숲 쪽으로 남음 |
| `cover_grass_temperate_weight` | temperate가 grass에 주는 비중 | 온대 초원 증가 | 기후 영향 약화 |
| `cover_shrub_aridity_weight` | aridity가 shrub에 주는 비중 | 건조 관목지 증가 | arid shrub 감소 |
| `cover_shrub_semi_arid_weight` | semi-arid가 shrub에 주는 비중 | 반건조 shrub 증가 | semi-arid 영향 약화 |
| `cover_shrub_hill_weight` | hill이 shrub에 주는 비중 | 구릉 관목지 증가 | hill 영향 약화 |
| `cover_shrub_bias` | shrub 기본 바닥값 | 전체적으로 shrub가 늘어남 | shrub가 드물어짐 |

### ecotone

| 필드 | 뜻 | 올리면 | 내리면 |
| --- | --- | --- | --- |
| `ecotone_thermal_weight` | thermal 경쟁이 ecotone에 주는 비중 | 기온대 경계가 더 잘 보임 | thermal 경계 영향 약화 |
| `ecotone_moisture_weight` | moisture 경쟁 비중 | 건습 전이대 강조 | moisture 경계 영향 약화 |
| `ecotone_overlay_weight` | overlay 경쟁 비중 | coast/river/wetland/alpine 전환 강조 | overlay 경계 영향 약화 |

---

## resolver

| 필드 | 뜻 | 올리면 | 내리면 |
| --- | --- | --- | --- |
| `ocean_landness_threshold` | preview에서 육지를 ocean으로 보지 않기 위한 최소 landness | coastline 근처가 ocean으로 남기 쉬움 | 얕은 육지도 육지 preview로 읽힘 |
| `overlay_min_strength` | coast/riverine/wetland/alpine overlay를 dominant로 인정하는 최소 강도 | overlay가 더 드물고 순수 biome가 많아짐 | overlay biome preview가 쉽게 등장 |
| `alpine_form_threshold` | mountain form이 alpine으로 넘어가는 추가 기준 | alpine이 더 드물어짐 | alpine이 더 쉽게 등장 |
| `coast_mountain_cap` | coast biome를 허용하는 mountain 상한 | 산악 해안이 coast로 남기 쉬움 | 조금만 험해도 coast 대신 mountain/other로 감 |
| `riverplain_wetness_threshold` | riverplain으로 읽히는 wetness 기준 | 진짜 습한 강가만 riverplain | riverplain이 더 흔해짐 |
| `forest_threshold` | forest biome preview로 읽는 forest potential 기준 | 숲 감소, 초원/스텝 증가 | 숲 증가 |
| `grass_threshold` | grassland로 읽는 grass potential 기준 | grassland 감소 | grassland 증가 |

---

## preview

### signed height

| 필드 | 뜻 | 올리면 | 내리면 |
| --- | --- | --- | --- |
| `ocean_depth_coast_weight` | 바다 signed depth에서 coast distance 비중 | 연안에서 멀어질수록 파랑 명암 차이 증가 | `landness` 기반 깊이감이 상대적으로 중요 |
| `ocean_depth_landness_weight` | 바다 signed depth에서 `(1 - landness)` 비중 | landness가 낮은 바다가 더 진해짐 | coast distance 기반 깊이감이 더 중요 |
| `land_height_macro_weight` | 육지 signed height에서 macro elevation 비중 | 높낮이 큰 육지가 더 진하게 보임 | mountain/ruggedness 영향 상대 증가 |
| `land_height_mountain_weight` | mountain mass 비중 | 산이 더 진한 녹색으로 강조 | elevation 중심 shading |
| `land_height_ruggedness_weight` | ruggedness 비중 | 험한 지형이 더 진하게 읽힘 | ruggedness shading 약화 |
| `land_height_alpine_weight` | alpine 비중 | 고산권 shading 강화 | alpine 색 차이 완화 |
| `land_height_coast_penalty` | coast factor가 육지 높이감을 깎는 정도 | 해안이 더 밝고 저지대로 보임 | 해안도 내륙처럼 진해짐 |

### 색과 명암

| 필드 | 뜻 | 올리면/바꾸면 | 내리면/반대로 바꾸면 |
| --- | --- | --- | --- |
| `ocean_light`, `ocean_dark` | ocean 밝은/어두운 끝색 | 바다 팔레트가 바뀜 | 바다 팔레트가 반대로 바뀜 |
| `ocean_shade_strength` | 바다 깊이에 따른 대비 | 깊은 바다와 얕은 바다 구분 강화 | ocean 색이 평평해짐 |
| `coast_light`, `coast_dark` | coast 밝은/어두운 끝색 | 해안 팔레트가 바뀜 | 해안 팔레트가 반대로 바뀜 |
| `coast_shade_strength` | 해안 높이감 대비 | 해안 내 명암 증가 | 해안 색이 평평해짐 |
| `polar_light`, `polar_dark` | polar 밝은/어두운 끝색 | 극지 팔레트가 바뀜 | 극지 팔레트가 반대로 바뀜 |
| `polar_shade_strength` | polar 명암 대비 | 극지 높이감 차이 증가 | 극지 흰색이 더 평평해짐 |
| `desert_light`, `desert_dark` | desert 밝은/어두운 끝색 | 사막 팔레트가 바뀜 | 사막 팔레트가 반대로 바뀜 |
| `desert_shade_strength` | desert 명암 대비 | dunes/highland 느낌 증가 | 사막색이 더 균일 |
| `wetland_light`, `wetland_dark` | wetland 색상 쌍 | 습지 팔레트가 바뀜 | 습지 팔레트가 반대로 바뀜 |
| `wetland_shade_strength` | wetland 명암 대비 | 습지 깊이감 증가 | 습지 색이 균일 |
| `riverplain_light`, `riverplain_dark` | riverplain 색상 쌍 | 강평야 팔레트가 바뀜 | 반대로 바뀜 |
| `riverplain_shade_strength` | riverplain 명암 대비 | 강평야 높낮이감 증가 | 색이 평평해짐 |
| `alpine_light`, `alpine_dark` | alpine 색상 쌍 | 고산 팔레트가 바뀜 | 반대로 바뀜 |
| `alpine_shade_strength` | alpine 명암 대비 | alpine relief 강조 | alpine 색이 평평 |
| `mountain_light`, `mountain_dark` | mountain 색상 쌍 | 산악 팔레트가 바뀜 | 반대로 바뀜 |
| `mountain_shade_strength` | mountain 명암 대비 | 높은 산과 낮은 산 차이 강화 | 산색이 균일 |
| `steppe_light`, `steppe_dark` | steppe 색상 쌍 | 스텝 팔레트가 바뀜 | 반대로 바뀜 |
| `steppe_shade_strength` | steppe 명암 대비 | 구릉 steppe 차이 증가 | steppe 색이 평평 |
| `grassland_light`, `grassland_dark` | grassland 색상 쌍 | 초원 팔레트가 바뀜 | 반대로 바뀜 |
| `grassland_shade_strength` | grassland 명암 대비 | 초원 relief 강조 | 초원색이 평평 |
| `temperate_forest_light`, `temperate_forest_dark` | temperate forest 색상 쌍 | 온대림 팔레트가 바뀜 | 반대로 바뀜 |
| `temperate_forest_shade_strength` | temperate forest 명암 대비 | 숲 지형 relief 강조 | 숲색이 평평 |
| `boreal_forest_light`, `boreal_forest_dark` | boreal forest 색상 쌍 | 냉대림 팔레트가 바뀜 | 반대로 바뀜 |
| `boreal_forest_shade_strength` | boreal forest 명암 대비 | boreal relief 강조 | 색이 평평 |
| `tropical_forest_light`, `tropical_forest_dark` | tropical forest 색상 쌍 | 열대림 팔레트가 바뀜 | 반대로 바뀜 |
| `tropical_forest_shade_strength` | tropical forest 명암 대비 | 열대림 relief 강조 | 색이 평평 |
| `river_tint` | riverine이 강한 육지 biome에 얹는 물빛 tint | 육지 biome에 섞이는 물색이 바뀜 | 다른 tint를 쓰게 됨 |
| `river_tint_threshold` | river tint를 얹기 시작하는 riverine 하한 | 큰 강가에만 tint | 작은 하천 근처도 tint |
| `river_tint_strength` | river tint 섞는 강도 | 강 주변이 더 푸르게 보임 | 기본 biome 색이 더 남음 |

---

## 추천 사용 순서

보통은 아래 순서로 만지는 게 덜 헷갈린다.

1. `normalization.land_threshold`
2. `continent.*`
3. `ridge.*`
4. `terrain.macro_*`, `terrain.mountain_*`
5. `hydrology.river_*`, `hydrology.lake_*`
6. `climate.*`
7. `resolver.*`
8. `preview.*`

처음 튜닝할 때는 `preview`보다 `continent / ridge / terrain / hydrology`를 먼저 잡는 편이 좋다.  
색은 보기 좋게 바뀌어도, 실제 필드가 나쁘면 atlas 품질이 좋아진 건 아니기 때문이다.
