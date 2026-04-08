# runner

## 역할

- 메인 루프의 소유자
- frame / fixed / shutdown 순서를 오케스트레이션한다

## 책임

- run() 진입점 제공
- 프레임 반복
- begin/end frame 경계 유지
- 종료 조건 확인
- shutdown 진입 시점 결정

## 비책임

- raw input 파싱
- ecs 내부 시스템 조합 구현
- simulation 알고리즘
- renderer draw 세부 구현

## 소유 데이터

- 루프 제어 상태
- running flag
- frame counter
- optional profiling hook

## 처리 흐름

1. 종료되지 않았으면 반복
2. frame 시작 준비
3. platform/frame pipeline 실행
4. fixed update 필요량 계산 및 실행
5. render 포함 frame 마무리
6. 종료 조건 검사
7. 필요 시 shutdown 실행

## 출력

- 프로그램 종료
- 종료 코드 또는 AppExitReason 반환 가능

## 상태 전이 규칙

- running 중에는 frame 순서를 항상 동일하게 유지
- exit_requested 감지 시 normal loop에서 shutdown phase로 전이
- shutdown 완료 시 loop 종료

## 불변식

- app의 frame 순서는 항상 동일해야 한다
- frame update와 fixed update는 명확히 분리된다
- catch-up 정책은 모든 프레임에서 동일하게 적용된다

## 관련 모듈

- frame.rs
- fixed.rs
- shutdown.rs
- state.rs

## 메모

- 초기에 runner 하나에 frame/fixed/shutdown이 모두 들어가도 되지만, 문서는 분리해두는 게 맞다