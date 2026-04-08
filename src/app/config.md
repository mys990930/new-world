# config

## 역할

- app가 사용하는 실행 정책을 typed config로 정의한다.
- 프레임 타이밍 정책을 코드 흐름과 분리한다.

## 소유 데이터

### AppConfig
- window title
- window width / height
- timing config

### TimingConfig
- `target_frame_rate: Option<u32>`

## 입력

- 하드코딩 기본값
- 향후 config file / CLI / 환경변수 결과

## 출력

- bootstrap 단계에서 쓰이는 typed config
- runner가 참고하는 frame timing policy

## 상태 전이 규칙

- bootstrap 이후 config는 immutable로 취급한다.
- runtime 중 자주 바뀌는 값은 config가 아니라 별도 runtime state로 둔다.

## 불변식

- `target_frame_rate = Some(n)`이면 `n > 0`인 값만 유효하다.
- `target_frame_rate = None`이면 frame cap 없이 poll cadence로 동작할 수 있다.

## 비책임

- config file 파싱
- validation error UI
- 실제 event loop control flow 구현

## 관련 모듈

- `bootstrap.rs`
- `state.rs`
- `runner.rs`

## 메모

- 현재 최소 구현에서 실제로 연결된 timing 값은 `target_frame_rate` 하나다.
- 기본값은 `Some(60)`이다.
