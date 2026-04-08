# camera

## 역할

- 게임 카메라의 gameplay 상태를 정의한다
- 4방향 쿼터뷰 회전, 느슨한 추적, 리센터 동작을 관리한다

## 소유 데이터

### CameraState
- current quarter-view direction
- follow target entity 또는 target position
- deadzone settings
- movement bias offset settings
- follow bias ratio
- current follow offset / smoothing state
- recenter in-progress state
- recenter speed policy

## 입력

- `RotateCamera` command
- `RecenterCamera` command
- 플레이어 transform / velocity
- 플레이어 이동 intent / 최근 진행 방향
- 좌/우클릭 상호작용 트리거
- 선택/가림 처리용 현재 camera orientation

## 출력

- 현재 카메라 방향 상태
- follow target 기준 카메라 위치/오프셋
- bias와 deadzone이 반영된 camera focus state
- selection / chunk / renderer bridge가 읽을 카메라 snapshot

## 상태 전이 규칙

- 카메라는 4방향 쿼터뷰만 제공한다
- `Q/E`는 항상 90도 단위 회전만 허용한다
- `WASD`는 플레이어 이동용 입력이며, 카메라는 그 입력에 직접 1:1로 묶여 움직이지 않는다
- 카메라는 플레이어를 느슨하게 추적한다
- 진행 방향 쪽에 더 많은 시야를 보여주도록, 플레이어는 화면상 진행 방향의 대략 `35%` 지점에 있고 나머지 `65%`는 전방 시야로 남기도록 bias를 줄 수 있다
- 플레이어가 이동 중일 때는 진행 방향 bias를 유지한다
- 플레이어가 멈추면 카메라는 천천히 center 쪽으로 복귀한다
- `Y`는 플레이어 기준 빠른 recenter 요청이다
- 좌/우클릭 상호작용 시에도 일반 정지 상태보다 더 빠른 recenter를 허용할 수 있다
- edge scroll은 지원하지 않는다

## 불변식

- 카메라는 RTS식 자유 팬이 아니라 편안한 추적 카메라다
- 회전 상태는 연속 yaw가 아니라 4방향 preset 상태다
- camera 모듈은 renderer의 GPU matrix 세부 구현을 알 필요가 없다
- camera 상태는 selection과 `MoveScreen -> MoveWorldIntent` 변환에 결정적으로 사용될 수 있어야 한다

## 비책임

- raw input 수집
- draw matrix 업로드
- world occlusion mesh 처리
- 블록 파괴/배치 적용

## 관련 모듈

- player.rs의 플레이어 상태를 따라간다
- selection.rs가 카메라 방향/위치를 기준으로 타겟을 판정한다
- app bridge 또는 renderer bridge가 카메라 snapshot을 외부 모듈에 전달할 수 있다

## 메모

- 화면 흔들림이나 줌 단계가 필요해지면 camera 상태에 확장 필드를 추가할 수 있다
- 추적/리센터 smoothing 파라미터는 프로토타입 중 조정 가능성이 높다
- `35 / 65` 시야 bias 비율은 프로토타입 중 조금 조정될 수 있다
