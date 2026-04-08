# window

## 역할

- 창 관련 상태를 보관하고, window 계열 platform event를 반영한다

## 소유 데이터

### WindowState
- physical_width: u32
- physical_height: u32
- scale_factor: f64
- focused: bool
- minimized: bool
- close_requested: bool
- resized_this_frame: bool

## 입력

- WindowResized
- ScaleFactorChanged
- FocusChanged
- MinimizedChanged
- CloseRequested

## 출력

- 현재 창 상태 조회
- 이번 프레임 resize 발생 여부 조회
- 현재 scale factor 조회

## 상태 전이 규칙

- WindowResized 수신 시 physical_width / physical_height 갱신
- ScaleFactorChanged 수신 시 scale_factor 갱신
- FocusChanged 수신 시 focused 갱신
- MinimizedChanged 수신 시 minimized 갱신
- CloseRequested 수신 시 close_requested = true
- resize가 발생한 프레임에는 resized_this_frame = true

## 프레임 경계 규칙

- begin_frame 시 resized_this_frame = false 로 초기화
- close_requested는 소비 전까지 유지하거나, 상위에서 처리 후 명시적으로 reset한다

## 불변식

- width/height는 0 이상이어야 한다
- window 모듈은 gameplay 의미를 만들지 않는다
- focused는 window focus만 의미하고, app active와 동일 개념이 아니다
- close_requested는 window close 의사 표현이지, 앱 종료 완료를 의미하지 않는다

## 비책임

- surface recreate 수행
- 렌더러 resize 처리
- pause 판단
- 입력 상태 정리
- 앱 전체 종료 처리

## 관련 모듈

- runtime.rs가 event를 공급
- renderer는 이 상태를 참고할 수 있음
- app이 close_requested를 보고 종료 정책을 결정할 수 있음

## 메모

- logical size가 필요해지면 별도 필드 추가 고려
- cursor visible / grab / confinement는 추후 분리 또는 확장 가능
