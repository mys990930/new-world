# jobs

## 역할

- jobs 시스템과 ECS 사이의 경계를 담당한다
- job 결과를 ECS 상태로 반영하고, ECS 상태를 바탕으로 후속 job 요청을 만든다

## 소유 데이터

### `ChunkStates`와의 연동 규칙
- generate-requested / mesh-requested 중복 방지
- loaded / render-ready 반영 규칙

## 입력

- `JobResult` queue
- chunk 메타 상태
- `WorldCore`

## 출력

- 갱신된 ECS meta state
- jobs 재제출용 request

## 상태 전이 규칙

- job 결과는 명시적으로 수거되기 전까지 보존된다
- 반영 순서는 deterministic해야 한다
- 같은 프레임에서 결과 반영과 후속 요청 생성의 순서는 고정되어야 한다
- generate/mesh 결과는 chunk meta state와 모순되지 않게 적용되어야 한다

## 불변식

- jobs 모듈은 실제 worker 실행을 소유하지 않는다
- ECS는 job 결과를 해석하지만 job 수행 자체를 하지 않는다
- world 원본 데이터 삽입/수정은 world API를 통해 이어져야 한다

## 비책임

- thread pool 실행
- file i/o
- meshing 구현
- simulation 계산 자체

## 관련 모듈

- chunk.rs와 함께 chunk 관련 요청/결과를 다룬다
- fixed.rs가 simulation 관련 job 결과와 이어진다
- app bridge가 jobs 시스템과의 실제 송수신을 연결할 수 있다

## 메모

- 현재 최소 구현에서 ECS는 player가 서 있는 청크 하나에 대해 `GenerateChunk -> BuildChunkMesh` 요청만 만든다
- 실제 `world.insert_chunk(...)`와 `renderer.apply_upload(...)`는 app가 수행하고, ECS는 요청/결과에 맞는 chunk meta만 갱신한다
