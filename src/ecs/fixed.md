# fixed

## 역할

- fixed tick에서만 실행되는 ECS 측 규칙과 simulation 연계를 담당한다
- active simulation region 계산과 simulation 요청/결과 흐름을 관리한다

## 소유 데이터

### SimClock
- logical simulation tick index
- fixed-step 관련 메타 상태

### ActiveSimRegion
- 현재 tick에서 활성화할 region / subsystem 범위

### SimulationControlState
- subsystem enable/disable 상태
- catch-up 관련 정책 메타

### PendingSimulationResults
- 아직 world/jobs 후속 단계로 넘기지 않은 simulation 결과

## 입력

- app이 결정한 fixed tick 실행 시점
- player 위치와 관심 영역
- chunk 메타 상태
- simulation 결과

## 출력

- simulation request
- world edit 후보
- dirty chunk / remesh / save 후속 요청

## 처리 흐름

1. app이 fixed phase 실행을 요청한다
2. ECS가 이번 tick의 활성 region / subsystem 대상을 계산한다
3. simulation 요청 또는 직접 실행 결과를 수집한다
4. 결과를 world/jobs/renderer 후속 단계에 넘길 intermediate state로 변환한다

## 상태 전이 규칙

- frame update와 fixed update는 섞이지 않는다
- simulation 대상 계산은 fixed phase에서만 수행한다
- 같은 tick의 결과 반영 순서는 deterministic해야 한다

## 불변식

- fixed timestep accumulator는 app이 소유한다
- ECS fixed 모듈은 simulation을 오케스트레이션하지만 simulation 알고리즘 자체를 구현하지 않는다
- world 원본 데이터 수정은 world API를 통해 이어져야 한다

## 비책임

- accumulator 관리
- simulation 세부 규칙 계산
- jobs 실행
- renderer draw

## 관련 모듈

- runtime.rs가 fixed schedule을 실행한다
- chunk.rs / jobs.rs / world / simulation과 맞닿는다
- player.rs 상태가 active region 계산의 기준이 될 수 있다

## 메모

- subsystem별 고정 업데이트 주기를 따로 둘지 여부는 나중에 확정 가능하다
