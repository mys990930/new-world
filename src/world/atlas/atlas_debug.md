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

## 불변식

1. 디버그 이미지는 같은 atlas 입력에서 동일한 픽셀 결과를 내야 한다.
2. 색상 램프와 class color는 tuning에 충분히 구분 가능해야 한다.
3. 저장 실패는 명시적 error로 반환한다.
