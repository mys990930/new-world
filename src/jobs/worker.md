# worker

## 역할

- 개별 worker 실행 단위와 shared `WorkerContext`를 정의한다.
- request 실행, 결과 보고, shutdown coordination을 담당한다.

## 책임

- worker thread/task lifecycle 관리
- request fetch 후 실행
- success/failure result 보고
- cancellation/shutdown signal 감시

## 비책임

- request 생성
- queue coalescing 정책
- 결과의 gameplay 의미 해석

## 소유 데이터

### WorkerContext
- thread pool 또는 task runtime handle
- request receiver / result sender
- shutdown flag 또는 cancellation token

### Worker-local state
- 현재 실행 중인 request 정보
- worker id / diagnostic metadata

## 입력

- queue가 전달한 `JobRequest`
- shutdown signal
- runtime-specific wakeup signal

## 출력

- `JobResult`
- worker idle/busy 상태 전이
- shutdown 완료 보고

## 처리 흐름

1. worker가 실행 가능한 request를 기다린다.
2. request를 받으면 현재 실행 중 상태로 전이한다.
3. `routing` 규칙에 따라 request를 실제 작업으로 실행한다.
4. success 또는 failure `JobResult`를 queue 쪽으로 보낸다.
5. idle 상태로 돌아가 다음 request를 기다린다.

## 불변식

- worker는 live world mutable state를 장기 보유하지 않는다.
- 하나의 request는 최대 한 worker만 실행한다.
- request 하나당 completion emission은 정확히 한 번이다.
- shutdown 정책이 graceful이면, 이미 시작한 request 처리 규칙이 명확해야 한다.

## 관련 모듈

- `runtime.md`
- `queue.md`
- `routing.md`

## 메모

- 실제 구현이 OS thread pool이든 async task runtime이든, 외부에서 보는 worker 계약은 동일하게 유지한다.
