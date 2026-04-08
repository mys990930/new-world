# player

## 역할

- 플레이어 중심 component와 이동 관련 상태 전이를 정의한다.
- 화면 기준 이동 상태를 world 기준 이동 intent로 변환한다.

## 소유 데이터

### Entity / Component
- `Player`
- `Transform`
- `Velocity`

### Resource / 보조 상태
- `LocalPlayerEntity`
- `MoveWorldIntent`

## 입력

- `EcsInputSnapshot`
- `CameraState`
- `PlayerCommandBuffer`
- local player entity id

## 출력

- `MoveWorldIntent`
- local player `Velocity`
- 향후 `Transform`, action state, chunk interest 계산 입력

## 상태 전이 규칙

- 화면 기준 상하좌우는 world 기준 동서남북과 일치하지 않는다.
- 기본 쿼터뷰에서 화면 우측 상단이 북쪽, 화면 우측 하단이 동쪽이다.
- 따라서 화면 기준 이동은 먼저 쿼터뷰 world axis로 투영한다.
- 그 다음 `CameraState.quarter_turns`를 반영해 현재 시점의 world 기준 이동 의도로 바꾼다.
- 같은 프레임에 `RotateCamera`가 들어오면 회전 후 기준으로 `MoveWorldIntent`를 계산한다.
- 현재 최소 구현에서는 `MoveWorldIntent`를 local player `Velocity`에 즉시 반영한다.

## 불변식

- player 모듈은 raw key state를 직접 읽지 않는다.
- `Transform.translation`은 현재 몸 중심 기준 위치로 해석한다.
- `MoveWorldIntent`는 continuous world-space movement 의미다.
- discrete action은 `PlayerCommandBuffer`, continuous movement는 `MoveWorldIntent`로 나뉜다.

## 비책임

- raw input 수집
- camera tracking 계산
- selection raycast
- world block edit

## 관련 모듈

- `input.rs`
- `command.rs`
- `camera.rs`
- `chunk.rs`

## 메모

- 현재 최소 구현에서 local player는 bootstrap 시점에 원점에 spawn된다.
- 현재는 velocity만 갱신하고 transform 적분은 아직 하지 않는다.
