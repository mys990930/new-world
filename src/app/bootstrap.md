# bootstrap

## 역할

- 프로그램 시작 시 필요한 상위 모듈을 생성하고 연결한다.

## 책임

- `AppConfig` 소비
- `Platform` 생성
- `Renderer` 생성
- `EcsRuntime` 생성
- app-owned timing state 생성
- 기본 local player spawn
- `GameApp` 조립

## 비책임

- 메인 루프 실행
- 프레임 반복
- fixed tick catch-up
- gameplay 규칙 처리

## 소유 데이터

- bootstrap 단계의 임시 생성값
- 초기 주입 순서

## 처리 흐름

1. config를 받는다.
2. platform을 생성한다.
3. config 기본값으로 renderer를 생성한다.
4. ECS runtime을 생성한다.
5. 기본 local player entity를 spawn한다.
6. config를 바탕으로 app timing state를 만든다.
7. `GameApp`을 조립한다.

## 출력

- 초기화된 `GameApp`

## 불변식

- 상위 모듈 생성과 dependency injection은 bootstrap이 담당한다.
- local player는 bootstrap 시점에 ECS world에 한 번만 생성한다.

## 관련 모듈

- `config.rs`
- `state.rs`
- `platform`
- `renderer`
- `ecs`

## 메모

- 현재 기본 player spawn은 원점 위치, zero velocity, 몸 중심 기준 `Transform`이다.
- 현재 renderer는 `StubSurfaceTarget`으로 먼저 생성되고, redraw 직전 실제 window 크기와 동기화된다.
