# frame

## 역할

- renderer frame render pass를 encode하고 submit/present를 수행한다.

## 책임

- frame 통계 계산
- camera GPU state 갱신
- camera uniform upload
- surface texture acquire
- clear pass와 dynamic cube draw
- submit / present
- recoverable surface error 전달

## 비책임

- render DTO 생성
- chunk visibility 계산
- gameplay state 해석

## 입력

- `RenderFrameInput`

## 출력

- `RenderStats`
- `RenderError`

## 처리 흐름

1. frame index와 stats를 갱신한다.
2. `RenderCameraState`를 `CameraGpuState`로 갱신한다.
3. live backend가 없으면 stats만 반환한다.
4. surface texture를 acquire한다.
5. camera uniform buffer를 업데이트한다.
6. `cube_instances`를 임시 cube mesh로 확장한다.
7. render pass에서 clear 후 cube를 draw한다.
8. submit / present 한다.

## 불변식

- renderer는 `RenderCubeInstance` 같은 render-ready DTO만 본다.
- live backend가 없거나 surface가 configure되지 않았으면 present하지 않는다.
- acquire 실패는 recoverable `RenderSurfaceError`로 상위에 전달한다.

## 관련 모듈

- `camera.rs`
- `surface.rs`
- `state.rs`

## 메모

- 현재 구현은 chunk draw보다 플레이어 큐브 가시화에 필요한 최소 dynamic draw path를 먼저 제공한다.
