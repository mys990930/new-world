# chunk

## 역할

- ECS가 소유하는 chunk 메타 상태를 관리한다
- visible / interest / loading / meshing / dirty / save_pending 흐름을 담당한다

## 소유 데이터

### ChunkStates
- currently interesting chunk set
- currently visible chunk set
- loading chunk set
- meshing chunk set
- dirty mesh set
- save pending set

### 보조 상태
- load/mesh/save 요청 억제용 중복 방지 상태

## 입력

- player 위치
- camera 상태
- world의 현재 chunk 존재 여부
- jobs 결과
- simulation/world 편집 후의 dirty 정보

## 출력

- `InterestChunks`
- `VisibleChunks`
- `ChunkLoadRequested`
- `ChunkMeshRequested`
- `ChunkSaveRequested`
- renderer / jobs / fixed가 참고할 chunk 메타 상태

## 상태 전이 규칙

- 플레이어 기준 interest 범위에 들어온 청크는 load 또는 generate 후보가 된다
- 카메라 기준 visible 범위는 renderer와 occlusion 판단에 사용된다
- interest와 visible은 서로 다른 집합일 수 있다
- 이미 로딩 중인 청크에 대해 중복 load 요청을 만들지 않는다
- dirty chunk는 meshing 후보가 되며, 필요한 이웃 조건을 만족할 때 mesh 요청을 만든다
- save pending 상태는 저장 완료 전까지 유지될 수 있다

## 불변식

- chunk 모듈은 원본 chunk 데이터 자체를 소유하지 않는다
- ECS는 chunk 메타 상태만 가진다
- interest 계산 기준은 플레이어/시뮬레이션 관점에서 결정적이어야 한다
- visible 계산 기준은 카메라/렌더 관점에서 결정적이어야 한다
- jobs 요청 생성 순서는 deterministic해야 한다

## 비책임

- chunk i/o
- 절차 생성 알고리즘
- meshing 알고리즘
- renderer draw

## 관련 모듈

- player.rs / camera.rs가 관심 영역 계산 기준을 제공한다
- jobs.rs가 결과 반영과 후속 요청을 조정한다
- world가 chunk 존재 여부와 snapshot을 제공한다

## 메모

- 초기 구현에서는 visible과 interest를 같은 값으로 시작할 수는 있지만, 리소스 경계는 처음부터 분리해 두는 편이 좋다
- 나중에 LOD나 distant chunk 표현이 생기면 세분화가 필요하다
