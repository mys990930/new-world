# scale

## 역할

- atlas 계층에서 사용하는 좌표와 하드 스케일 타입을 정의한다.

## 책임

- `AtlasCoord`
- `AtlasArea`
- atlas cell 크기와 region/chunk 관계 상수
- atlas grid index 규칙

## 하드 스케일

- `1 atlas cell = 256m x 256m`
- `1 atlas cell = 16 x 16 chunk columns`
- `1 atlas cell = 2 x 2 regions`

## 불변식

1. atlas area의 width / height는 0보다 커야 한다.
2. atlas grid index는 `(x, z)`에서 결정적으로 계산 가능해야 한다.
3. atlas 좌표는 chunk generation 여부와 무관하게 독립 조회 가능해야 한다.
