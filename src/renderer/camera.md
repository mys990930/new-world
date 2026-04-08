# camera

## 역할

- renderer가 요구하는 카메라 GPU 표현을 관리한다
- app/ecs가 넘긴 카메라 snapshot을 uniform으로 반영한다

## 책임

- `RenderCameraState`를 view / projection 행렬로 변환
- camera uniform buffer 생성 / 갱신
- camera bind group 생성 / 갱신
- surface 크기 변화에 따른 aspect / projection 반영
- frame draw 전에 최신 camera GPU 상태 준비

## 비책임

- 카메라 이동 입력 해석
- gameplay camera mode 판단
- visible chunk 계산
- render pass 실행

## 소유 데이터

- `CameraGpuState`
- current aspect ratio
- projection config cache
- last uploaded camera snapshot

## 입력

- app/bridge가 만든 `RenderCameraState`
- current drawable size
- `CameraProjectionConfig`
- camera bind group layout

## 출력

- frame.rs가 바로 사용할 camera uniform / bind group
- resize 후 갱신된 projection state

## 상태 전이 규칙

- 초기화 시 uniform buffer와 bind group을 생성한다
- resize 시 aspect ratio와 projection cache를 갱신한다
- 프레임마다 필요한 경우 camera uniform을 다시 업로드한다

## 불변식

- renderer는 gameplay camera의 source of truth가 아니다
- projection 계산은 현재 surface 크기와 일관되어야 한다
- camera upload는 해당 프레임 draw 전에 끝나야 한다

## 관련 모듈

- config.rs가 projection 기본값을 제공
- pipeline.rs가 bind group layout을 제공
- state.rs가 `CameraGpuState`를 소유
- frame.rs가 draw 직전에 camera state를 사용한다
- app::bridge가 gameplay camera를 render camera로 번역한다

## 메모

- orthographic / perspective 전환이 필요해지면 `RenderCameraState`에 mode를 추가할 수 있다
- shadow cascade 같은 확장은 별도 카메라 계열 모듈로 분리 가능하다

