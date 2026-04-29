# pipeline

## 역할

`pipeline`은 graph-first world generation의 stage order와 column synthesis scaffold를 정의한다.

이 모듈은 새 파이프라인이 legacy generator를 대체하기 전까지 compile-time stage contract를
제공한다. 실제 stage 구현은 `graph`, `macro_map`, `hydrology`, `boundary`, `field`,
`heightfield`, `surface_plan`, `voxel`, `preview` 문서와 구현으로 분산된다.

---

## 책임

- graph-first generation stage 이름 정의
- initial generation config surface 정의
- chunk voxelization이 나중에 소비할 column synthesis request/result shape 정의
- stage 순서가 macro guide, hydrology, heightfield, voxel fill 순서를 어기지 않도록 고정

---

## 비책임

- Voronoi site 실제 생성
- noise function 실행
- hydrology solving
- surface material 선택
- `ChunkData`에 block 배치

---

## Stage Order

현재 scaffold stage:

1. macro Voronoi graph
2. continuous region fields
3. land/ocean gradient
4. mountain gradient
5. hydrology graph
6. biome resolve
7. heightfield synthesis
8. voxel fill

목표 pipeline은 더 세분화될 수 있지만, 반드시 아래 대원칙을 지켜야 한다.

- graph construction이 먼저다.
- continent/ocean과 macro elevation은 Perlin보다 먼저다.
- mountain/ridge/fault/coast edge guide는 hydrology보다 먼저다.
- hydrology는 final heightfield와 voxel fill보다 먼저다.
- noisy boundary와 Voronoi-derived macro map은 visible artifact를 숨기는 중간 layer다.
- Perlin micro relief는 마지막 표면 디테일이며 macro ownership을 뒤집지 않는다.

---

## 불변식

1. chunk coordinate는 output window만 고르며 macro terrain identity를 정의하지 않는다.
2. graph와 hydrology guide는 base heightfield가 voxel로 확정되기 전에 계산되어야 한다.
3. sea-level contract는 `WorldMeta.generator_version`이 의도적으로 바꾸기 전까지 world-space `y = 0`을 유지한다.
4. stage output은 preview와 test가 같은 deterministic 입력으로 재현할 수 있어야 한다.

---

## 현재 구현 상태

- 현재는 v2 pipeline compile-time scaffold 단계다.
- legacy generation entrypoint는 migration 동안 `world::generation`을 통해 re-export된다.
