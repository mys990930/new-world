# camera

## 역할

- gameplay 관점의 편안한 추적 카메라 상태를 정의한다
- 4방향 고정 쿼터뷰 회전, 느슨한 follow, deadzone, 진행 방향 bias, smooth recenter 규칙을 관리한다
- movement / selection / render bridge가 공유하는 quarter-view basis와 follow pose helper를 제공한다

## 소유 데이터

### CameraState
- `quarter_turns`
- current smoothed follow target
- current desired follow target
- recenter 진행 상태 또는 요청 상태
- deadzone / bias / follow smoothing을 일관되게 유지하는 데 필요한 내부 상태

### QuarterViewBasis
- `right`
- `up`
- `forward`

## 입력

- `RotateCamera`
- `RecenterCamera`
- local player `Transform`
- `MoveWorldIntent`
- 향후 player velocity / interaction context

## 출력

- 현재 카메라 회전 상태와 follow pose
- `MoveWorldIntent` 계산에 필요한 orientation basis
- selection ray origin/direction 계산에 필요한 basis
- app bridge가 renderer용 `RenderCameraState`를 만들 때 쓰는 camera snapshot

## 상태 전이 규칙

- 카메라는 4방향 고정 쿼터뷰 preset만 제공한다
- RTS식 자유 팬은 제공하지 않는다
- `Q/E`는 항상 90도 단위 회전이다
- 같은 프레임의 회전은 그 프레임 이동 intent 계산 전에 적용된다
- 플레이어는 deadzone 내부에서는 카메라를 바로 끌고 가지 않는다
- 플레이어가 deadzone 밖으로 벗어나면 카메라는 플레이어를 deadzone 안쪽으로 되돌릴 만큼만 target을 갱신한다
- target 갱신은 hard snap이 아니라 부드러운 follow smoothing으로 접근한다
- 진행 방향 bias는 현재 `MoveWorldIntent` 기준으로 작게만 적용된다
- 진행 방향 bias는 플레이어를 화면에서 진행 방향 쪽으로 약간 치우치게 두어, 진행 앞쪽 월드를 더 많이 보여주는 방향으로 적용한다
- 진행 방향 bias는 player center를 대체하지 않고, 정지하거나 방향이 바뀌면 다시 약해진다
- `Y`는 회전값을 바꾸지 않고, 카메라를 player-centered anchor 쪽으로 부드럽게 lerp 복귀시키는 recenter 요청이다
- selection과 render는 같은 프레임에 같은 smoothed target과 basis를 사용해야 한다

## 좌표계 규칙

- 창 기준 좌표계와 world 기준 좌표계는 45도 어긋나 보인다
- 기본 쿼터뷰에서:
  - 화면 우측 상단 = world north
  - 화면 우측 하단 = world east
- 따라서 player 이동 해석과 selection ray 생성은 camera orientation을 거친 world axis 변환이 필요하다
- deadzone과 진행 방향 bias는 raw world axis가 아니라 quarter-view의 `right/up` 평면 기준으로 평가되어야 네 방향 회전에서 일관된다

## 불변식

- camera 모듈은 renderer의 GPU matrix를 직접 계산하지 않는다
- camera orientation과 smoothed follow pose는 selection과 movement intent 변환, render bridge의 공통 기준이 된다
- app bridge와 selection은 local player transform만으로 별도의 카메라 target을 다시 만들지 않는다
- `quarter_view_basis()`와 follow pose helper는 movement, selection, render bridge가 같은 방향 규칙을 공유하도록 유지한다

## 비책임

- raw input 수집
- world edit apply
- renderer draw matrix 계산
- 자유 팬/자유 비행 카메라 제공

## 관련 모듈

- `command.rs`
- `player.rs`
- `selection.rs`
- `chunk.rs`
- `app/bridge.rs`

## 메모

- 이 문서는 편안한 추적 카메라의 목표 계약을 정의한다.
- 현재 코드는 아직 `quarter_turns`와 raw player-centered target 기반의 단순 쿼터뷰 카메라만 구현한 상태다.
- orthographic framing은 플레이 공간을 조금 더 넓게 읽을 수 있도록 너무 타이트하지 않게 유지한다.
- 구현은 `CameraState`와 shared follow pose helper를 확장하는 방향으로 맞추고, gameplay camera 규칙을 renderer 쪽으로 밀어 넣지 않는다.
