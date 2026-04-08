# command

## 역할

- ECS 내부와 외부 경계에서 쓰이는 discrete gameplay command와 continuous movement intent를 정의한다.
- 입력 해석과 실제 상태 전이를 분리하는 staging 경계를 제공한다.

## 소유 데이터

### PlayerCommand
- `PrimaryAction`
- `PlaceBlock`
- `RotateCamera { quarter_turns }`
- `RecenterCamera`

### MoveWorldIntent
- `east`
- `north`

### PlayerCommandBuffer
- 현재 프레임에 생성된 discrete player command 목록

## 입력

- input 단계가 생성한 discrete command
- camera / player / selection 단계가 소비할 buffer

## 출력

- camera/player/selection이 소비하는 discrete command
- player가 생성하는 world 기준 이동 intent
- 향후 network DTO로 내려갈 수 있는 gameplay 의미

## 생성 규칙

- `MoveScreen`은 더 이상 command가 아니다.
- 화면 기준 이동 상태는 `EcsInputSnapshot`에 남는다.
- discrete 행동만 `PlayerCommandBuffer`로 들어간다.
- `MoveWorldIntent`는 camera orientation을 반영한 뒤 player 단계에서 만든다.

## 소비 규칙

- command buffer는 프레임 경계에서 clear된다.
- 같은 프레임에 생성된 `RotateCamera`는 그 프레임의 `MoveWorldIntent` 계산 전에 반영된다.
- `MoveWorldIntent`는 continuous movement state이며 drain 대상이 아니다.

## 순서 규칙

- 같은 프레임의 처리 순서는 deterministic해야 한다.
- 현재 update 순서는:
  - input interpretation
  - camera command 적용
  - world intent 생성
  - local player velocity 반영

## 불변식

- discrete 행동과 continuous movement intent는 같은 버퍼에 섞지 않는다.
- `MoveWorldIntent`는 창 기준이 아니라 world 기준 의미다.
- 멀티플레이 경계에서는 `MoveWorldIntent` 같은 world 기준 표현이 더 안정적이다.

## 비책임

- raw input 수집
- world edit apply
- renderer upload

## 관련 모듈

- `input.rs`
- `camera.rs`
- `player.rs`

## 메모

- 향후 block break/place, interact, use item 같은 discrete command는 여기서 늘어난다.
