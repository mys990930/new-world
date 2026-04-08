# bootstrap

## 역할

- 프로그램 시작 시 필요한 상위 모듈을 생성하고 연결한다

## 책임

- 설정 로드/검증
- platform 초기화
- renderer 초기화
- world / ecs / jobs / simulation 초기화
- 모듈 간 초기 dependency injection
- GameApp 생성

## 비책임

- 메인 루프 실행
- 프레임 반복 처리
- fixed tick 반복 실행
- gameplay 규칙 판단

## 소유 데이터

- bootstrap 단계의 임시 생성 값
- 초기 설정값
- 초기 리소스 주입값

## 처리 흐름

1. config 로드
2. platform 생성
3. renderer 생성
4. world 생성
5. jobs 생성
6. ecs 생성 및 초기 resource 주입
7. simulation 생성
8. GameApp 조립

## 출력

- GameApp::new(...) 결과
- 초기화가 끝난 런타임 인스턴스

## 불변식

- 초기화 순서는 platform/window/surface 선행 조건을 깨지 않아야 한다
- dependency injection은 bootstrap에서 끝내고, 이후 하위 모듈이 서로를 직접 생성하지 않는다
- 플랫폼 차이로 인한 초기화 분기는 가능한 bootstrap과 platform 내부에 국한한다

## 관련 모듈

- config.rs
- state.rs
- platform
- renderer
- ecs
- world
- jobs
- simulation

## 메모

- 초기 세이브 로드, 플레이어 스폰, 초기 청크 warm-up도 추후 여기 또는 별도 startup_phase로 둘 수 있다
- 현재 최소 구현은 `Platform`과 `EcsRuntime`만 실제로 생성/주입한다
- 현재 최소 구현에서는 bootstrap 시점에 local player entity도 함께 생성한다
