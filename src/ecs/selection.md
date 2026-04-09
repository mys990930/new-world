# selection

## 역할

- 현재 커서가 가리키는 world block과 face를 판정한다
- 상호작용/파괴/배치로 이어질 수 있는 최소 selection 상태를 보관한다
- renderer가 노란 face highlight를 그릴 수 있는 gameplay snapshot을 제공한다

## 소유 데이터

### SelectionState
- `hovered_block`
- `hovered_face`
- `hit_point`

## 입력

- `EcsInputSnapshot.cursor_screen_pos`
- `EcsInputSnapshot.focused`
- `EcsInputSnapshot.active`
- current `CameraState.quarter_turns`
- local player `Transform`
- viewport width/height
- `WorldCore::raycast_blocks(...)` 결과

## 출력

- 현재 hover block
- 현재 hover face
- hit point
- app bridge가 render highlight로 번역할 수 있는 최소 selection snapshot

## 상태 전이 규칙

- app frame은 world/job 결과 반영 뒤 현재 viewport와 world state를 기준으로 selection을 갱신한다
- selection ray는 current quarter-view basis와 cursor screen position으로부터 orthographic 방식으로 계산한다
- raycast가 solid block에 닿으면 `hovered_block`, `hovered_face`, `hit_point`를 채운다
- hit가 없거나 viewport/cursor/focus 조건이 유효하지 않으면 selection을 clear한다

## 불변식

- selection은 타겟 판정 상태를 소유하지만 world를 직접 수정하지 않는다
- selection은 world 내부 블록 step 알고리즘을 직접 소유하지 않고 world query surface를 사용한다
- renderer는 `SelectionState`를 직접 읽지 않고 app bridge가 만든 render-ready cube instance만 받는다

## 비책임

- world raycast 알고리즘 자체
- 반투명 렌더링 구현
- 실제 block edit apply
- raw input 수집
- 앞/뒤 타겟 hover 전환 정책의 완성 구현

## 관련 모듈

- `camera.rs`의 방향 helper를 읽는다
- `player.rs`의 local player transform을 읽는다
- `runtime.rs`가 selection update 진입점을 제공한다
- `world`가 raycast hit를 제공하고 `app/bridge.rs`가 렌더용 노란 face highlight로 바꾼다

## 메모

- 현재 구현은 최소 vertical slice만 포함한다.
  - stored state: `hovered_block`, `hovered_face`, `hit_point`
  - render output: hovered face 위의 얇은 노란 slab
- 뒤쪽 타겟 우선, 앞쪽 hover `0.3s` 전환, placement preview position 분리, occluder 후보 추적은 아직 future work다.
