# shutdown

## 역할

- 종료 요청 이후의 안전한 종료 절차를 관리한다

## 책임

- close_requested / quit_requested 확인
- shutdown 진입 여부 결정
- 필요한 flush/drain 수행
- 자원 정리 순서 실행
- 종료 완료 상태로 전이

## 비책임

- 일반 frame update 실행
- 게임 저장 포맷 구현
- 플랫폼 이벤트 수집 자체
- world 내부 정합성 구현

## 입력

- platform/window/lifecycle 종료 신호
- pending job 상태
- save required 상태
- renderer/device 자원 상태

## 출력

- 종료 완료
- optional exit reason / exit code

## 처리 흐름

1. 종료 요청 감지
2. 새 작업 접수 중단 또는 제한
3. 필요 시 저장 flush
4. 필요 시 jobs drain 또는 cancel
5. renderer/platform 자원 정리
6. 종료 완료 상태 전이

## 상태 전이 규칙

- normal running -> exit requested -> shutdown in progress -> terminated
- 종료 절차 중에는 일반 gameplay update를 더 이상 진행하지 않는다

## 불변식

- 종료 순서는 항상 예측 가능해야 한다
- 데이터 유실 허용 범위는 정책적으로 명확해야 한다
- shutdown은 app이 소유하고, 개별 모듈은 자기 정리만 수행한다

## 관련 모듈

- runner.rs
- state.rs
- platform
- jobs
- world
- renderer

## 메모

- 초기 버전에서는 shutdown.rs를 runner.rs 안에 합쳐도 된다
- 하지만 저장/백그라운드 작업이 들어가면 분리하는 게 맞다