# input

## 역할

- app가 전달한 platform 기반 frame snapshot을 ECS가 읽을 수 있는 입력 resource로 유지한다.
- discrete 행동과 continuous 이동 상태를 분리된 형태로 해석한다.

## 소유 데이터

### EcsInputSnapshot
- `move_screen_x`
- `move_screen_y`
- `primary_down`
- `primary_just_pressed`
- `secondary_down`
- `secondary_just_pressed`
- `rotate_camera`
- `recenter_camera`
- `cursor_screen_pos`
- `cursor_screen_delta`
- `focused`
- `active`

## 입력

- app bridge가 생성한 frame 입력 snapshot

## 출력

- `PlayerCommandBuffer`에 push되는 discrete command
- `player.rs`가 읽는 화면 기준 이동 상태

## 상태 전이 규칙

- `move_screen_x / move_screen_y`는 화면 기준 이동 의도다.
- 이 값은 카메라 이동이 아니라 플레이어 이동 상태를 뜻한다.
- 이동은 command로 push하지 않고 snapshot에 남겨둔다.
- `primary_just_pressed`는 `PrimaryAction`
- `secondary_just_pressed`는 `PlaceBlock`
- `rotate_camera`는 `RotateCamera`
- `recenter_camera`는 `RecenterCamera`
- `active == false` 또는 `focused == false`면 일반 gameplay command를 만들지 않는다.

## 프레임 경계 규칙

- snapshot은 app가 프레임마다 최신 값으로 교체 주입한다.
- `*_just_pressed`는 다음 프레임 accumulation 전에만 유효하다.
- hold 상태는 `*_down`과 `move_screen_x/y`에 남아 있다.

## 불변식

- input 모듈은 raw OS event를 직접 다루지 않는다.
- 화면 기준 이동 상태와 world 기준 이동 intent는 여기서 합쳐지지 않는다.
- input 모듈은 world를 직접 수정하지 않는다.

## 비책임

- raw input 수집
- world axis 변환
- camera tracking 계산
- selection raycast

## 관련 모듈

- `app/bridge.rs`
- `command.rs`
- `player.rs`
- `camera.rs`

## 메모

- 현재 최소 구현에서 input 단계는 discrete command만 `PlayerCommandBuffer`에 넣는다.
- `RecenterCamera`의 실제 smoothing 규칙은 input이 아니라 `camera.rs`가 소유한다.
