# input

## 역할

- 마우스/키보드의 raw input 상태를 보관하고, 프레임 단위 transient 상태를 관리한다

## 소유 데이터

### RawInputState
- mouse_screen_pos: Vec2
- mouse_delta: Vec2
- wheel_delta: Vec2
- left_pressed / right_pressed
- left_just_pressed / right_just_pressed
- left_just_released / right_just_released
- pressed_keys
- just_pressed_keys
- just_released_keys
- modifiers
- text_input_buffer

## 입력

- CursorMoved
- MouseButtonChanged
- MouseWheel
- KeyChanged
- ModifiersChanged
- TextInput
- FocusChanged(false) 또는 명시적 InputClearRequested
- Suspended

## 출력

- 현재 pressed 상태 조회
- just_pressed / just_released 조회
- mouse position / delta 조회
- wheel delta 조회
- 이번 프레임 text input 조회

## 상태 전이 규칙

- MouseButtonChanged(button, true)
  - 해당 버튼의 pressed = true
  - 기존에 눌려있지 않았다면 해당 just_pressed = true
- MouseButtonChanged(button, false)
  - 해당 버튼의 pressed = false
  - 이전에 눌려 있었다면 해당 just_released = true
- KeyChanged 도 동일 규칙 적용
- CursorMoved 는 mouse_screen_pos 갱신
- CursorMoved 간 차이를 mouse_delta 에 누적
- MouseWheel 은 wheel_delta 누적
- ModifiersChanged 는 modifier snapshot 갱신
- TextInput 은 text_input_buffer 에 append

## 프레임 경계 규칙

- begin_frame 시 아래 transient 상태를 초기화한다
  - mouse_delta
  - wheel_delta
  - just_pressed_*
  - just_released_*
  - text_input_buffer
- pressed_* 상태는 유지된다

## 포커스 상실 규칙

- 포커스를 잃으면 stuck input 방지를 위해 pressed / just_pressed / just_released 를 모두 정리할 수 있어야 한다
- suspend 시에도 같은 안전 정리를 적용할 수 있다
- 이 동작은 gameplay 의미가 아니라 raw safety 처리다

## 불변식

- input 모듈은 raw state만 다룬다
- 좌클릭 = 상호작용 같은 의미는 만들지 않는다
- just_pressed / just_released 는 한 프레임만 유효하다
- mouse_delta / wheel_delta 는 프레임 누적값이다

## 비책임

- 카메라 회전 해석
- 이동 명령 생성
- 블럭 설치 의도 판정
- UI focus 정책 판단
- 입력 매핑(binding) 정책

## 관련 모듈

- runtime.rs가 event를 공급
- ecs/app이 snapshot을 읽고 gameplay command로 해석

## 메모

- 키 repeat를 별도 처리할지 추후 결정
- 마우스 raw delta와 screen pos를 둘 다 유지할지 정책 확정 필요
- gamepad 지원 시 입력 소스 확장 가능
