# camera

## 역할

- render camera DTO를 GPU가 사용할 수 있는 camera state와 uniform으로 변환한다.

## 책임

- `RenderCameraState` 정의
- `RenderProjectionMode`, `RenderViewBasis` 정의
- view / projection / view_projection 계산
- resize 시 aspect ratio cache 갱신
- `CameraUniform` 생성

## 비책임

- gameplay camera rule 결정
- camera follow / deadzone / bias 계산
- draw pass encode

## 입력

- `RenderCameraState`
- `CameraProjectionConfig`
- surface width / height

## 출력

- `CameraGpuState`
- `CameraUniform`

## 불변식

- renderer는 최종 `eye/target/up`만 받고 gameplay camera 의미는 알지 않는다.
- 필요하면 app bridge가 explicit view basis와 projection mode를 render-ready DTO로 넘길 수 있다.
- CPU 쪽 matrix는 row-major로 계산하고, uniform upload 직전 WGSL column-major에 맞게 transpose한다.

## 관련 모듈

- `frame.rs`
- `surface.rs`
- `config.rs`

## 메모

- 현재 prototype은 quarter-view 디버그 가시성을 위해 orthographic projection과 explicit basis override를 사용한다.
