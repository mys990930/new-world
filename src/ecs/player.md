# player

## 역할

- 플레이어 중심 component와 상태 전이를 정의한다
- 이동, 행동, 입력 소비 결과를 플레이어 상태에 반영한다

## 소유 데이터

### Entity / Component
- Player
- Transform
- Velocity
- MoveWorldIntent
- Inventory
- Health

### Resource 또는 보조 상태
- local player entity id
- 플레이어 상태 전이용 임시 intent/resource

## 입력

- `PlayerCommandBuffer`
- camera의 현재 4방향 회전 상태
- world query 결과(충돌, 지면 여부 등)
- fixed/frame dt

## 출력

- `MoveWorldIntent`
- 갱신된 플레이어 transform / velocity / action state
- camera follow target 정보
- chunk interest 계산용 기준 위치
- interaction/selection의 기준 위치와 방향

## 상태 전이 규칙

- `MoveScreen` command는 카메라 4방향 회전 상태를 기준으로 `MoveWorldIntent`로 변환된다
- 플레이어 실제 이동은 `MoveWorldIntent`를 소비한 결과로 반영된다
- 이동은 raw key state가 아니라 command 소비 결과로 반영된다
- 행동 상태 전이는 플레이어 resource/component 안에서만 일어난다
- 플레이어 위치 변화는 이후 camera/chunk/fixed 단계의 입력이 된다

## 불변식

- 플레이어 상태는 raw input을 직접 읽지 않는다
- world 원본 블록 데이터는 player 모듈이 소유하지 않는다
- 화면 기준 이동과 world 기준 이동의 변환은 camera 상태를 기준으로 결정적이어야 한다
- 멀티플레이 경계에서 재현 가능한 movement 의미는 `MoveWorldIntent` 쪽에 있어야 한다
- 로컬 플레이어와 일반 엔티티 구조는 가능하면 동일한 ECS 패턴을 유지한다

## 비책임

- raw input 수집
- camera 추적 계산
- 타겟 raycast
- world block 편집
- jobs 실행

## 관련 모듈

- command.rs의 command를 소비한다
- camera.rs가 플레이어 상태를 따라간다
- selection.rs가 플레이어 기준 상호작용 범위를 읽는다
- chunk.rs / fixed.rs가 플레이어 위치를 interest 영역 계산에 사용한다

## 메모

- 초반에는 local player 하나만 두더라도, 구조는 멀티플레이 확장을 고려해 두는 게 좋다
- animation state가 생기면 player.md 범위를 더 세분화할 수 있다
