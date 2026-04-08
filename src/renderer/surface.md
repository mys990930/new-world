# surface

## 역할

- renderer 초기화의 surface-facing state를 담당한다
- drawable size / configured 상태 / resize lifecycle을 관리한다

## 책임

- `RenderSurfaceTarget` trait 경계 제공
- drawable size snapshot 획득
- 초기 configured / minimized 상태 판정
- resize 시 surface state 재구성
- resize / present generation bookkeeping
- zero-sized window / minimized 상태에 대한 안전 처리

## 비책임

- 실제 `wgpu` instance / surface / device / queue 생성
- shader module 생성
- render pipeline layout 구성
- CPU mesh 업로드
- draw call 순서 결정
- gameplay 카메라 계산

## 소유 데이터

- `RenderSurfaceTarget`
- `SurfaceSnapshot`
- configured / minimized 상태
- resize generation
- present generation

## 처리 흐름

1. `RenderSurfaceTarget`에서 drawable size를 읽는다
2. `SurfaceSnapshot`과 configured 상태를 초기화한다
3. `Renderer` 생성 시 pipeline / camera가 참조할 surface state를 제공한다
4. resize 이벤트 시 width / height를 반영한다
5. present 성공 시 present generation을 증가시킨다

## 출력

- `SurfaceState`
- resize 후 일관된 drawable size / configured 상태
- 이후 실제 backend가 붙을 수 있는 surface lifecycle 슬롯

## 불변식

- width 또는 height가 0인 동안에는 무의미한 configure / render를 강제하지 않는다
- platform-specific window 세부사항은 `RenderSurfaceTarget` 경계 안에 캡슐화한다
- resize bookkeeping과 camera/pipeline 재동기화 시점은 deterministic해야 한다

## 관련 모듈

- config.rs가 정책값을 제공
- state.rs가 `SurfaceState`를 소유
- pipeline.rs가 surface format / depth format을 참조
- frame.rs가 current texture acquire와 error 처리를 수행
- app bootstrap이 초기 생성 순서를 조율한다
- platform이 window handle을 제공한다

## 메모

- 현재 1차 구현은 실제 `wgpu` surface/device/queue를 만들지 않고, 다음 단계에서 그 자리에 실제 backend를 붙일 수 있게 상태 경계만 먼저 만든다
- 최소 구현에서는 단일 window / 단일 surface만 가정한다
