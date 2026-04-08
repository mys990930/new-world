# camera

## 역할

- gameplay 관점의 카메라 상태를 정의한다.
- 4방향 쿼터뷰 회전과 recenter 관련 신호를 관리한다.

## 소유 데이터

### CameraState
- `quarter_turns`
- `recenter_boost_requested`

## 입력

- `RotateCamera`
- `RecenterCamera`
- `PrimaryAction`
- `PlaceBlock`
- 향후 player transform / velocity / move intent

## 출력

- 현재 카메라 회전 상태
- `MoveWorldIntent` 계산에 필요한 orientation basis
- 향후 renderer / selection / chunk가 참조할 camera snapshot

## 상태 전이 규칙

- 카메라는 4방향 쿼터뷰 preset만 제공한다.
- `Q/E`는 항상 90도 단위 회전이다.
- 같은 프레임의 회전은 그 프레임 이동 intent 계산 전에 적용된다.
- `Y`는 빠른 recenter 1회성 boost 신호다.
- `좌클릭`, `우클릭` 상호작용도 빠른 recenter 1회성 boost 신호를 만든다.
- boost는 지속 모드가 아니라 해당 프레임에만 의미를 가진다.
- slow tracking, deadzone, 35/65 진행 방향 bias는 향후 camera follow 구현에서 확장한다.

## 좌표계 규칙

- 창 기준 좌표계와 world 기준 좌표계는 45도 어긋나 보인다.
- 기본 쿼터뷰에서:
  - 화면 우측 상단 = world north
  - 화면 우측 하단 = world east
- 따라서 player 이동 해석은 camera orientation을 거친 world axis 변환이 필요하다.

## 불변식

- camera 모듈은 renderer의 GPU matrix를 직접 계산하지 않는다.
- camera orientation은 selection과 movement intent 변환의 기준이 된다.

## 비책임

- raw input 수집
- world edit apply
- renderer draw matrix 업로드

## 관련 모듈

- `command.rs`
- `player.rs`
- `selection.rs`
- `chunk.rs`

## 메모

- 현재 최소 구현은 `quarter_turns`와 `recenter_boost_requested`만 실제 코드로 연결되어 있다.
