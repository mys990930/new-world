# world_scale.md

## 1. 문서 목적

이 문서는 월드 생성과 런타임 전반에서 사용하는 공간 스케일의 기준 단위를 정의한다.

목표는 다음과 같다.

1. 블록, 청크, 리전, 아틀라스 셀의 관계를 고정한다.
2. 월드 생성, 생태계, 스트리밍, 메싱, 렌더링이 같은 스케일 체계를 공유하게 한다.
3. 이후 `world_atlas.md`, `biome_resolver.md`, `chunk_realization.md`의 기준 좌표계를 제공한다.

이 문서에서 정의하는 값은 크게 두 종류다.

- 하드 스케일: 구조 자체를 바꾸는 값. 초기에 고정해야 한다.
- 소프트 스케일: 노이즈/분포 파라미터. 이후 프로토타입에서 계속 조정 가능하다.

---

## 2. 핵심 원칙

### 2.1 블록 디테일 우선
이 프로젝트는 1m 복셀보다 더 자잘한 환경 디테일과 건축 디테일을 목표로 한다.
따라서 기본 블록 크기는 0.5m로 둔다.

### 2.2 청크는 편집/생성/메싱의 실질 단위
청크는 단순 저장 단위가 아니라,
- 절차 생성
- dirty 추적
- 메싱
- GPU 업로드
- 스트리밍
의 실질 단위가 된다.

### 2.3 지역 셀은 생태/장기 상태 단위
청크는 즉시 보이는 상태를,
지역 셀은 장기 생태 상태를 담당한다.
따라서 region은 청크보다 한 단계 큰 단위로 둔다.

### 2.4 atlas는 청크 이전의 거시 월드 해석 단위
atlas는 실제 블록을 채우기 전,
월드의 대륙/해양/산맥/기후/바이옴 분포를 해석하는 상위 단위다.

---

## 3. 기본 단위 정의

## 3.1 Block
- 1 block = 0.5m
- 최소 건축/지형 표현 단위
- 충돌, 배치, 파괴, 재질 표현의 기본 단위

### 선택 이유
- 마인크래프트보다 더 촘촘한 건축/지형 디테일 확보
- 소형 구조물, 계단형 경사, 강둑, 절벽 표현력 향상
- 플레이어가 “풍경과 어울리는 집”을 짓는 데 더 유리한 해상도 제공

---

## 3.2 Chunk Section
- 1 chunk section = 32 × 32 × 32 blocks
- 물리 크기 = 16m × 16m × 16m

### 선택 이유
- 16³은 0.5m 블록 기준으로 물리 크기가 너무 작아 청크 수가 과도해진다
- 64³은 블록 수정, 리메시, 부분 갱신 범위가 너무 커진다
- 32³은 편집 단위, 스트리밍 단위, 메싱 단위로 가장 균형이 좋다

### chunk section의 역할
- 절차 생성 결과 저장 단위
- dirty/remesh 추적 단위
- CPU mesh 생성 단위
- GPU mesh 업로드 단위

---

## 3.3 Vertical Stack / Column
- 월드는 세로로 여러 개의 chunk section을 쌓는 구조를 사용한다
- 하나의 column은 `(chunk_x, chunk_z)` 기준 수직 stack이다

예시:

column(x, z)
- section_y = 0
- section_y = 1
- section_y = 2
- ...

### 선택 이유
- 높은 산, 협곡, 동굴, 절벽을 수직 단위로 분리 가능
- 일부 높이만 dirty/remesh 가능
- 통짜 고정 높이 청크보다 수정/생성/메싱 효율이 좋다

---

## 3.4 World Height
초기 권장안:
- vertical sections = 32
- total block height = 1024 blocks
- total physical height = 512m

### 의미
- 언덕과 진짜 산을 분리 가능
- 고산 냉대와 일반 산기슭을 분리할 수 있는 높이 여유 확보
- 절벽, 계곡, 고원, 하천 고도차 표현 가능

---

## 3.5 Region Cell
- 1 region = 8 × 8 chunk columns
- 물리 크기 = 128m × 128m

### region의 역할
- 장기 생태 상태 저장
- 초식 압력 / 포식 압력 / 식생 회복력 / 인간 활동 압력 저장
- region 평균 기후 및 계절 보정 기준 단위
- 오프스크린 경량 시뮬레이션 단위

### 선택 이유
- chunk보다 큰 장기 상태 단위가 필요함
- 너무 작으면 chunk와 차별이 약해짐
- 너무 크면 강가/숲가/산기슭처럼 중요한 변화를 뭉개게 됨

---

## 3.6 Atlas Cell
- 1 atlas cell = 2 × 2 regions
- 1 atlas cell = 16 × 16 chunk columns
- 물리 크기 = 256m × 256m

### atlas의 역할
- 대륙/해양 마스크 계산
- 거시 고도장 계산
- 산맥/능선장 계산
- 강 잠재력 및 수계 구조 계산
- 온도/습도/내륙성 같은 기후장 계산
- macro biome weights 계산
- biome map / climate map / ridge map 같은 디버그 출력 단위

### 선택 이유
- 청크보다 충분히 커서 거시 필드를 보기 쉽다
- region보다 한 단계 커서 atlas와 생태 region 역할을 분리할 수 있다
- 디버그 지도로 보기에도 촘촘하고, worldgen 상위 해상도로도 적절하다

---

## 4. 단위 관계 요약

- 1 block = 0.5m
- 1 chunk section = 32³ blocks = 16m cube
- 1 region = 8 × 8 chunk columns = 128m
- 1 atlas cell = 2 × 2 regions = 256m
- 1 world column = (x, z) 기준 vertical stack

---

## 5. 하드 스케일과 소프트 스케일

## 5.1 하드 스케일
초기에 고정해야 하는 값:
- block_size_m = 0.5
- chunk_section_size = 32
- vertical_sections = 32
- region_size_in_chunks = 8
- atlas_size_in_regions = 2

## 5.2 소프트 스케일
이후 계속 조정 가능한 값:
- ocean coverage
- continent size
- mountain chain length
- river density
- lake frequency
- temperature scale
- humidity scale
- inland dryness strength
- alpine threshold
- ecotone width

---

## 6. 불변식

1. 월드 원본 데이터 단위는 block / chunk section 기준으로 관리한다.
2. GPU 메쉬 업로드/교체는 chunk section coord 기준으로 동작한다.
3. 장기 생태 상태는 region 기준으로 관리한다.
4. atlas cell은 청크 생성 이전의 거시 월드 해석 단위다.
5. 하드 스케일은 초기에 자주 바꾸지 않는다.

---

## 7. 추후 문서와의 연결

- `world_generation.md`
  - 전체 레이어 개요와 문서 링크만 유지
- `world_atlas.md`
  - atlas cell 기준 거시 필드 계산 정의
- `biome_resolver.md`
  - atlas field → biome weights / ecotone 해석
- `chunk_realization.md`
  - atlas / biome 결과 → 실제 블록/재질/식생 채우기