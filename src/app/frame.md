# frame

## 역할

- frame update 파이프라인을 정의하고 실행한다

## 책임

- platform frame begin/end 호출
- OS/window/input 이벤트 수집 단계 실행
- platform 상태를 게임 쪽으로 연결
- ecs frame phase 실행
- jobs 결과 수거
- renderer upload/render 호출

## 비책임

- fixed timestep accumulator 관리
- simulation 알고리즘 실행
- world 내부 데이터 정합성 구현
- raw input 저장

## 입력

- GameApp
- platform snapshot
- frame dt
- jobs completion
- renderer pending upload state

## 출력

- 갱신된 ecs/world/jobs/renderer 상태
- 필요 시 world 변경 요청, jobs 요청, render 요청 생성

## 처리 흐름

1. platform.begin_frame()
2. platform 이벤트 수집
3. bridge를 통해 platform raw state를 ecs resource로 반영
4. ecs pre/update/post 실행
5. jobs 완료 결과 수거 및 반영
6. renderer upload/render
7. platform.end_frame()

## 현재 구현 메모

- 현재 최소 vertical slice에서는 platform snapshot을 읽고 frame counter를 증가시키는 수준까지만 구현되어 있다
- bridge / ecs / jobs / renderer 연결은 이후 frame.rs 안에서 단계적으로 확장한다

## 불변식

- platform은 raw state만 제공하고 gameplay 의미는 ecs가 만든다
- jobs는 frame 안에서 요청/결과 수거만 수행한다
- renderer는 world source of truth가 아니다
- frame 단계에서 fixed simulation 규칙을 직접 계산하지 않는다

## 비책임

- fixed tick 반복
- close 후 flush/drain 처리
- 시스템 상세 등록 규칙 관리

## 관련 모듈

- bridge.rs
- runner.rs
- platform
- ecs
- jobs
- renderer

## 메모

- jobs 결과 수거 시점을 ecs post 뒤로 둘지, 별도 apply phase를 둘지는 이후 확정 가능
