# input

## 역할

- app이 전달한 platform 기반 입력 스냅샷을 ECS가 읽을 수 있는 frame resource로 보관한다
- 화면 기준 조작을 gameplay command 후보로 해석한다

## 소유 데이터

### EcsInputSnapshot
- move_screen_x: i8
- move_screen_y: i8
- primary_down
- primary_just_pressed
- secondary_down
- secondary_just_pressed
- rotate_camera
- recenter_camera
- cursor_screen_pos
- cursor_screen_delta
- focused
- active

## 입력

- app bridge가 생성한 frame 입력 snapshot
- platform raw state에서 이미 정규화된 값

## 출력

- `PlayerCommandBuffer`에 push되는 command 후보
- 이후 selection / camera / player 시스템이 읽을 frame 입력 resource

## 상태 전이 규칙

- `move_screen_x / move_screen_y`는 화면 기준 이동 의도를 나타낸다
- 이 값은 카메라 이동이 아니라 플레이어 이동 의도를 뜻한다
- `primary_just_pressed`는 기본 행위 command 후보를 만든다
- `secondary_just_pressed`는 블록 배치 command 후보를 만든다
- `rotate_camera`는 `Q/E`에 의해 `-1 / +1 / 0`으로 표현된다
- `recenter_camera`는 `Y` 입력에 의해 한 프레임 동안만 활성화된다
- `active == false` 또는 `focused == false`이면 일반 gameplay command를 만들지 않는다

## 프레임 경계 규칙

- 입력 snapshot은 app이 프레임마다 새 값으로 교체 주입한다
- `*_just_pressed`류는 한 프레임만 유효하다고 가정한다
- hold 상태(`*_down`)는 app bridge가 유지/갱신한 값을 그대로 따른다

## 불변식

- ECS input은 raw OS 이벤트가 아니라 app이 정규화한 frame snapshot만 다룬다
- 이동 기준은 항상 화면 기준이다
- 입력 단계에서는 camera follow 위치를 직접 계산하지 않는다
- input 모듈은 command 후보를 만들 수는 있어도 world를 직접 수정하지 않는다
- input 모듈은 카메라 실제 위치 계산이나 타겟 판정을 직접 수행하지 않는다

## 비책임

- raw input 수집
- 키 바인딩 UI 정책
- world 편집 적용
- 카메라 추적 계산
- 타겟 raycast

## 관련 모듈

- app/bridge.rs가 `EcsInputSnapshot`을 주입한다
- command.rs의 `PlayerCommandBuffer`를 채운다
- camera.rs / selection.rs / player.rs가 입력 snapshot을 읽는다

## 메모

- 입력 매핑이 복잡해지면 action map 계층을 추가할 수 있다
- gamepad 지원 시 같은 snapshot 형식으로 입력 소스를 합칠 수 있다
