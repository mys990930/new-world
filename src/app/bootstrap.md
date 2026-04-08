# bootstrap

## 역할

- 앱 시작 시 필요한 상위 모듈을 생성하고 연결한다.

## 책임

- `AppConfig` 준비
- `Platform` 생성
- `Renderer` 생성
- `EcsRuntime` 생성
- 기본 local player spawn
- `AppTimingState` 생성
- `GameApp` 조립

## 비책임

- 메인 루프 실행
- redraw scheduling
- fixed tick 반복
- gameplay rule 처리

## 처리 흐름

1. config를 받는다.
2. `Platform`을 만든다.
3. renderer는 아직 OS window가 없으므로 `StubSurfaceTarget`으로 먼저 생성한다.
4. `EcsRuntime`을 만든다.
5. 기본 local player entity를 spawn한다.
6. app timing state를 만든다.
7. `GameApp`을 반환한다.

## 출력

- 초기화된 `GameApp`

## 불변식

- bootstrap 시점 renderer는 반드시 생성되지만, live GPU backend는 아직 없을 수 있다.
- local player는 bootstrap 시점에 한 번만 spawn한다.

## 관련 모듈

- `config.rs`
- `state.rs`
- `platform`
- `renderer`
- `ecs`

## 메모

- live window surface attach는 bootstrap이 아니라 `runner.rs`의 `resumed()`에서 한다.
