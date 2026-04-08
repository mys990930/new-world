# frame

## 역할

- renderer의 frame render pass를 정의하고 실행한다

## 책임

- current surface texture acquire
- command encoder / render pass 생성
- clear color / depth clear 적용
- camera GPU state 갱신 호출
- visible chunk draw call 기록
- queue submit / present
- surface error를 분류해 상위에 전달

## 비책임

- 메인 루프 소유
- fixed timestep 관리
- meshing / upload 요청 생성
- gameplay 입력 해석
- world 수정

## 입력

- `RenderFrameInput`
- `Renderer`
- visible chunk 목록
- optional debug draw 입력

## 출력

- present된 프레임
- `RenderStats`
- recoverable `RenderError`

## 처리 흐름

1. drawable size가 유효한지 확인한다
2. current surface texture를 acquire 한다
3. camera.rs를 통해 최신 camera uniform을 반영한다
4. command encoder와 render pass를 연다
5. visible chunk 순회하며 존재하는 `GpuChunkMesh`만 draw 한다
6. submit 후 present 한다
7. surface error면 재시도 / resize 필요 여부를 상위로 전달한다

## 현재 구현 메모

- 현재 저장소의 `frame.rs`는 실제 command encoder / submit / present 대신, camera 갱신과 visible mesh 집계 및 `RenderStats` 생성까지만 수행한다

## 불변식

- 동일 입력에 대한 draw 순서는 deterministic해야 한다
- present는 성공적인 submit 이후에만 수행한다
- frame 단계는 gameplay 의미나 world source data를 만들지 않는다
- surface acquire 실패나 outdated 상태는 조용히 삼키지 않고 상위에 전달한다

## 관련 모듈

- surface.rs가 acquire / present 대상 surface를 제공
- pipeline.rs가 draw pipeline을 제공
- camera.rs가 camera uniform을 준비한다
- upload.rs가 채운 GPU cache를 사용한다
- app::frame가 renderer 호출 순서를 조율한다
- platform::window가 resize / minimized 상태 정보를 제공할 수 있다

## 메모

- 초기에는 chunk pass 하나만 있어도 되지만, 이후 transparent / entity / debug overlay pass가 늘어날 수 있다
- render graph가 필요해질 정도로 pass가 복잡해지면 별도 상위 구조를 도입한다
