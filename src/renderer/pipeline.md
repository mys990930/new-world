# pipeline

## 역할

- renderer가 사용하는 shader / layout / render pipeline을 생성하고 유지한다

## 책임

- shader module 로드 / 생성
- bind group layout 정의
- pipeline layout 생성
- render pipeline 생성
- surface format / sample count 변경 시 재구성 판단
- draw 단계가 사용할 pipeline handle 제공

## 비책임

- CPU mesh 생성
- camera 움직임 계산
- frame loop 소유
- visible chunk 결정
- world 데이터 변환 정책 판단

## 소유 데이터

- shader module handle
- bind group layout
- pipeline layout
- opaque chunk pipeline
- optional debug / wireframe pipeline

## 입력

- `RenderConfig`
- current surface format
- current depth format
- camera / material binding contract

## 출력

- `PipelineSet`
- frame.rs가 사용할 draw-ready pipeline handle
- camera.rs가 사용할 bind group layout 정보

## 처리 흐름

1. surface 초기화가 끝난 뒤 format / sample count를 확인한다
2. shader module을 준비한다
3. bind group layout / pipeline layout을 만든다
4. render pipeline을 생성한다
5. 호환성이 깨지면 pipeline을 재생성한다

## 불변식

- pipeline은 현재 surface format / depth format과 호환되어야 한다
- shader-specific binding 세부사항은 pipeline 경계 안에 캡슐화한다
- binding contract가 바뀌면 관련 bind group / resource도 함께 갱신되어야 한다

## 관련 모듈

- config.rs가 sample / debug 정책을 제공
- state.rs가 `PipelineSet`을 저장
- camera.rs가 camera bind group layout을 사용
- frame.rs가 실제 draw 시 pipeline을 바인딩한다
- surface.rs가 format 정보를 제공한다

## 메모

- 초기에 opaque voxel chunk pipeline 하나만 있어도 문서 구조는 분리해두는 게 좋다
- shader hot-reload가 필요해지면 `pipeline.rs` 또는 별도 `shader.rs` 분리를 고려할 수 있다

