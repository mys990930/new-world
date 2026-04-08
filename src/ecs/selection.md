# selection

## 역할

- 현재 상호작용/파괴/배치 타겟을 판정한다
- 가림 처리와 배치 프리뷰에 필요한 gameplay 상태를 관리한다

## 소유 데이터

### SelectionState
- current target block/entity
- current hit face
- placement preview target position
- front occluder candidate
- front-hover timer
- target transition state

### InteractionTarget
- block 또는 entity target 식별 정보
- interaction validity

## 입력

- cursor screen position
- camera orientation / follow state
- player position / interaction range
- world query / raycast 결과
- `PrimaryAction` / `PlaceBlock` command

## 출력

- 현재 상호작용 타겟
- 현재 배치 프리뷰 위치/면
- world request로 변환 가능한 target 정보
- 가림 처리 대상 후보

## 상태 전이 규칙

- 기본적으로는 뒤쪽의 유효 타겟을 우선 선택한다
- 플레이어 앞의 가림 블록을 수정하려는 경우, 특정 면 위에 커서를 약 `0.3s` 이상 머무르면 앞쪽 타겟으로 전환 가능해야 한다
- 블록 배치는 커서가 가리키는 노출된 면 기준으로, 기존 블록에 인접한 위치에 배치한다
- 프리뷰는 설치될 위치에 반투명으로 표시되며, 어느 면에 붙는지 분명해야 한다
- 원하는 면이 보이지 않으면 카메라 회전으로 해결하는 흐름을 전제로 한다

## 가림 처리 규칙

- 카메라 기준으로 플레이어를 실제로 가리는 최소 블록만 반투명 처리한다
- 앞쪽 전체를 일괄 반투명 처리하지 않는다
- 반투명 상태 블록은 시야 확보용으로 동작하며, 뒤쪽 타겟 우선 판정과 함께 사용된다

## 불변식

- selection은 타겟 판정과 프리뷰 상태를 소유하지만 world를 직접 수정하지 않는다
- 기본 타겟 우선순위는 뒤쪽 유효 타겟이다
- 앞쪽 전환은 의도적 hover를 통해서만 허용된다
- placement preview는 실제 배치 결과와 일관돼야 한다

## 비책임

- world raycast 알고리즘 자체
- 반투명 렌더링 구현
- 실제 block edit apply
- raw input 수집

## 관련 모듈

- camera.rs의 방향/상태를 읽는다
- player.rs의 위치/범위를 읽는다
- command.rs의 상호작용/배치 command를 소비한다
- world가 query 결과를 제공하고 renderer가 preview/occlusion 표현을 담당할 수 있다

## 메모

- hover 전환 시간 `0.3s`는 프로토타입 중 조정 가능성이 높다
- entity interaction과 block interaction을 분리할지 여부는 이후 확장 포인트다
