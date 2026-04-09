# atlas_debug

## 역할

- atlas field와 resolved preview를 디버그 이미지로 저장한다.

## 책임

- scalar map rasterization
- categorical map rasterization
- atlas prototype에서 필요한 파일명 규칙 유지

## 기본 출력 묶음

- `00_landness.png`
- `01_elevation.png`
- `02_ridge.png`
- `03_hydrology.png`
- `04_temperature.png`
- `05_humidity.png`
- `06_overlay.png`
- `07_biome_preview.png`
- `08_ecotone.png`

## biome preview 색 규칙

- `07_biome_preview.png`는 atlas cell 단위 preview biome를 색으로 읽기 쉽게 표현한다.
- 바다는 파랑 계열을 기본으로 하되, 해수면 아래로 더 깊게 읽히는 셀일수록 더 짙은 파랑으로 표현한다.
- 육지는 초록 계열을 기본으로 하되, 해수면 위로 더 높게 읽히는 셀일수록 더 짙은 초록으로 표현한다.
- 해안은 연노랑을 우선 사용한다.
- 사막 계열은 주황을 우선 사용한다.
- 극지 계열은 흰색을 우선 사용한다.
- 강가/습지/고산/숲 계열은 위 기본 규칙 위에서 별도 tint를 섞어 구분한다.
- 여기서 쓰는 높이감은 atlas preview용 signed height이며, 실제 chunk block 고도와 동일한 계약은 아니다.

## 불변식

1. 디버그 이미지는 같은 atlas 입력에서 동일한 픽셀 결과를 내야 한다.
2. 색상 램프와 class color는 tuning에 충분히 구분 가능해야 한다.
3. 저장 실패는 명시적 error로 반환한다.
4. biome preview의 signed height와 주요 색 규칙은 `tuning.rs`의 preview 섹션에서 조정 가능해야 한다.
