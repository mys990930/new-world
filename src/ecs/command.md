# command

## 역할

- ECS 내부와 외부 경계에서 사용하는 gameplay command / request 타입을 정의한다
- 입력 해석과 실제 상태 전이 사이를 분리하는 buffer를 제공한다

## 소유 데이터

### PlayerCommand
- `MoveScreen { x, y }`
- `PrimaryAction`
- `PlaceBlock`
- `RotateCamera { quarter_turns }`
- `RecenterCamera`

### MoveWorldIntent
- quarter-view 방향을 반영한 world 기준 이동 intent
- 서버 authoritative movement와 prediction/replay의 기준이 되는 입력 의미

### PlayerCommandBuffer
- 현재 프레임에 생성된 player command 목록

## 입력

- input.rs가 해석한 frame 입력
- 이후 selection / player / camera 시스템이 만드는 추가 command

## 출력

- player.rs / camera.rs / selection.rs가 소비하는 gameplay command
- player.rs가 생성하는 `MoveWorldIntent`
- 필요 시 network 전송용 command DTO의 입력

## 생성 규칙

- 한 command는 하나의 gameplay 의미만 표현한다
- raw 키 상태 대신 gameplay 의미로 생성한다
- 멀티플레이를 고려해 platform 세부 구현 타입을 담지 않는다
- 화면 기준 입력과 world 기준 이동 의도는 별도 단계로 분리한다

## 소비 규칙

- command buffer는 프레임 경계에서 명시적으로 초기화된다
- 입력 해석 phase가 command를 채우고, 이후 phase가 소비한다
- `MoveScreen`은 player.rs에서 camera 4방향 상태를 반영한 `MoveWorldIntent`로 변환된다
- 소비 후 상위 계층이 drain 하거나 다음 단계 request로 변환한다

## 순서 규칙

- 같은 프레임 내 command 생성 순서는 deterministic해야 한다
- camera 회전, 이동, 상호작용 command의 상대 순서는 정책적으로 고정되어야 한다
- 네트워크 전송 대상이 되는 command는 추후 sequence/tick 정보를 덧붙일 수 있어야 한다
- 멀티플레이 경계에서는 가능하면 world 기준 intent를 사용하는 편이 좋다

## 불변식

- command는 raw input이 아니다
- command는 world API 호출과 동일하지 않다
- command 타입은 app/platform 내부 구현을 몰라야 한다
- command buffer는 한 프레임 단위의 임시 staging 영역이다
- `MoveScreen`은 클라이언트 표현 계층 의미이고, `MoveWorldIntent`는 시뮬레이션/권위 계층 의미다

## 비책임

- raw input 수집
- world edit 적용
- GPU 업로드
- 네트워크 전송 수행

## 관련 모듈

- input.rs가 command를 생성한다
- player.rs / camera.rs / selection.rs가 command를 소비한다
- network.md와 맞닿는 전송용 DTO의 기반이 될 수 있다

## 메모

- 앞으로 block break/place, interact, use item 같은 command를 더 세분화할 수 있다
- command와 request를 분리할지 여부는 world/jobs 연결 단계에서 다시 검토 가능하다
- `MoveWorldIntent`를 네트워크 전송 단위로 직접 쓸지, 그보다 상위 client command 래퍼를 둘지는 이후 확정 가능하다
