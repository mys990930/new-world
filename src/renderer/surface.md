# surface

## 역할

- renderer 초기화의 GPU context 부분을 담당한다
- surface / device / queue lifecycle과 resize 처리를 관리한다

## 책임

- instance / surface / adapter / device / queue 생성
- surface capability 조회와 config 선택
- 초기 surface configure
- resize 시 surface reconfigure
- depth texture 생성 / 재생성
- zero-sized window / minimized 상태에 대한 안전 처리

## 비책임

- shader module 생성
- render pipeline layout 구성
- CPU mesh 업로드
- draw call 순서 결정
- gameplay 카메라 계산

## 소유 데이터

- window / surface 생성 입력
- adapter capability snapshot
- surface config
- depth texture / depth view
- optional surface recreate 필요 플래그

## 처리 흐름

1. platform window handle에서 surface 생성 입력을 받는다
2. adapter / device / queue를 초기화한다
3. capability를 보고 surface format / present mode / alpha mode를 선택한다
4. surface.configure(...)를 호출한다
5. drawable size에 맞는 depth texture를 만든다
6. resize 이벤트 시 width / height를 검증한 뒤 surface / depth를 재설정한다

## 출력

- `SurfaceState`
- resize 후 일관된 drawable size / depth 상태
- recoverable surface error를 상위로 전달할 수 있는 상태 정보

## 불변식

- surface / device / queue는 서로 호환 가능한 조합이어야 한다
- width 또는 height가 0인 동안에는 무의미한 configure / render를 강제하지 않는다
- surface format이 바뀌면 pipeline 호환성 재검토가 필요하다
- platform-specific window 세부사항은 surface 경계 안에 캡슐화한다

## 관련 모듈

- config.rs가 정책값을 제공
- state.rs가 `SurfaceState`를 소유
- pipeline.rs가 surface format / depth format을 참조
- frame.rs가 current texture acquire와 error 처리를 수행
- app bootstrap이 초기 생성 순서를 조율한다
- platform이 window handle을 제공한다

## 메모

- device lost / surface lost recovery를 어디까지 자동으로 처리할지는 이후 정책 확정 필요
- 최소 구현에서는 단일 window / 단일 surface만 가정한다

