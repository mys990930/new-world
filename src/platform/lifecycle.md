# lifecycle

## 역할

- 앱 생명주기 관련 상태를 추적하고 lifecycle 계열 event를 반영한다

## 소유 데이터

### LifecycleState
- active: bool
- suspended: bool
- quit_requested: bool

## 입력

- ActiveChanged
- Suspended
- Resumed
- QuitRequested

## 출력

- 현재 active 여부
- suspended 여부
- quit 요청 여부

## 상태 전이 규칙

- ActiveChanged(active) 수신 시 active 갱신
- Suspended 수신 시 suspended = true, active = false
- Resumed 수신 시 suspended = false
- QuitRequested 수신 시 quit_requested = true

## 의미 규약

- active는 앱이 입력/실행 가능한 상태인지에 대한 lifecycle 수준 신호다
- focused와 동일하지 않다
- quit_requested는 종료 의사 표시이며, 종료 완료 상태가 아니다

## 불변식

- lifecycle은 플랫폼 생명주기만 다룬다
- pause menu 진입 여부 같은 게임 의미는 만들지 않는다
- 앱 종료 정책 결정은 상위 계층 책임이다

## 비책임

- game paused 상태 결정
- 저장 후 종료 여부 결정
- window close 처리 자체
- 입력 정리

## 관련 모듈

- runtime.rs가 lifecycle event를 공급
- app이 quit_requested를 보고 종료 순서를 결정
- input.rs는 필요 시 focus/lifecycle 변화에 반응해 상태를 정리할 수 있음

## 메모

- 모바일 대응 시 background/foreground 상태를 더 세분화할 수 있다
- desktop only 초기 버전이면 active/suspended 구분을 단순화해도 된다
