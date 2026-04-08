# event

## 역할

- platform 내부에서 사용하는 정규화된 이벤트 타입 정의
- OS/winit 이벤트를 하위 상태 모듈이 소비 가능한 공통 형태로 표현

## 핵심 원칙

- event는 raw platform signal만 표현한다
- gameplay 의미는 담지 않는다
- event 자체는 상태를 저장하지 않는다

## 이벤트 목록

### window 계열
- WindowResized { physical_width, physical_height }
- ScaleFactorChanged { scale_factor }
- FocusChanged { focused }
- MinimizedChanged { minimized }
- CloseRequested

### input 계열
- CursorMoved { x, y }
- MouseMotion { dx, dy }
- MouseButtonChanged { button, pressed }
- MouseWheel { delta_x, delta_y }
- KeyChanged { key, pressed }
- ModifiersChanged { shift, ctrl, alt, logo }
- TextInput { text }

### lifecycle 계열
- ActiveChanged { active }
- Suspended
- Resumed
- QuitRequested

## 정규화 규칙

- runtime은 winit/OS 이벤트를 가능한 한 lossless하게 PlatformEvent로 변환한다
- 하위 모듈이 직접 winit 타입을 알 필요 없도록 한다
- 플랫폼별 차이는 event 단계 또는 runtime 내부에서 흡수한다

## 비책임

- pressed/just_pressed 상태 저장
- WindowState 저장
- LifecycleState 저장
- 게임 pause 판단
- player input 의미 해석

## 관련 모듈

- runtime.rs가 생성한다
- window.rs / input.rs / lifecycle.rs가 소비한다

## 메모
