# config

## 역할

- renderer가 사용하는 설정 구조를 정의한다
- surface / pipeline / frame 정책값을 코드와 분리한다

## 소유 데이터

### RenderConfig
- preferred present mode
- preferred surface format policy
- depth format
- clear color
- sample count
- upload budget / staging policy
- optional debug rendering toggle

### CameraProjectionConfig
- vertical_fov
- near_plane
- far_plane

## 입력

- app bootstrap에서 넘기는 renderer 설정
- 하드코딩 기본값
- 플랫폼 / GPU capability에 따른 선택 정책

## 출력

- surface.rs가 사용할 configure 정책
- pipeline.rs가 사용할 format / sample count 정책
- frame.rs / camera.rs가 사용할 clear color / projection 기본값

## 상태 전이 규칙

- config는 로드 후 immutable하게 유지하는 것을 기본으로 한다
- 실제 surface format / present mode 선택 결과는 runtime state에 저장하고, config 원본은 정책값으로 남긴다
- 런타임 중 변경 가능한 그래픽 옵션이 필요하면 별도 runtime settings로 분리한다

## 불변식

- sample count는 지원 가능한 합법 값이어야 한다
- near / far plane은 유효한 projection 범위를 만들어야 한다
- config는 GPU runtime state가 아니라 초기화 / 실행 정책이다

## 비책임

- adapter capability 조회 구현
- surface configure 호출
- shader / pipeline 생성
- 카메라 이동 계산

## 관련 모듈

- surface.rs가 `RenderConfig`를 소비
- pipeline.rs가 format / sample count 정책을 소비
- camera.rs가 projection 기본 설정을 소비
- app bootstrap이 renderer config를 조립한다

## 메모

- vsync / unlocked present mode 전환이 필요해지면 `RenderConfig`에서 정책만 노출하고 실제 전환은 surface.rs가 수행한다
- 품질 프리셋이 커지면 `DebugRenderConfig`, `PostProcessConfig` 같은 하위 구조로 쪼갤 수 있다
- projection kind 자체는 현재 `RenderCameraState`가 frame 단위 render-ready DTO로 override할 수 있다
