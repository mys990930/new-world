# config

## 역할

- app이 사용하는 설정 구조를 정의한다
- 런타임 정책값을 코드와 분리한다

## 소유 데이터

### AppConfig
- window / platform 관련 설정
- renderer 초기화 설정
- world / jobs / ecs / simulation 초기 설정
- debug option
- save/load 관련 상위 정책

### TimingConfig
- target_frame_rate (optional)
- fixed_dt
- max_fixed_steps_per_frame
- time_scale (optional)

### SimulationConfig
- subsystem enable/disable
- simulation region policy
- tick budget 관련 설정

## 입력

- 설정 파일 로드 결과
- CLI/환경변수/디버그 오버라이드 값
- 하드코딩 기본값

## 출력

- bootstrap 단계에서 사용할 typed config
- runner / fixed 단계에서 사용할 timing policy

## 상태 전이 규칙

- config는 로드 후 immutable하게 유지하는 것을 기본으로 한다
- 런타임 중 변경 가능한 옵션이 필요하면 별도 runtime settings로 분리한다

## 불변식

- fixed_dt는 0보다 커야 한다
- max_fixed_steps_per_frame는 무한 catch-up을 막을 수 있어야 한다
- config는 도메인 상태가 아니라 실행 정책이다

## 비책임

- 설정 파일 파싱 구현 세부
- 설정 검증 실패 후 사용자 UI 처리
- 실제 시스템 초기화

## 관련 모듈

- bootstrap.rs가 config를 소비
- fixed.rs가 TimingConfig를 사용
- runner.rs가 종료/루프 정책 일부를 참조할 수 있음

## 메모

- dev/prod preset 분리 가능
- 나중에 network 설정이 들어오면 별도 NetworkConfig 추가 가능