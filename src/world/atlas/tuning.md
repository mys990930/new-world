# tuning

## 역할

- atlas prototype의 조절 가능한 기본값을 한 곳에 모은다.
- generation, resolver, biome preview debug가 어떤 수치를 기준으로 동작하는지 한눈에 보이게 한다.

## 목표

- "이 값을 바꾸면 대륙 크기/산맥 빈도/강 밀도/기후 분포/preview 색이 어떻게 달라지겠구나"를 한 파일에서 파악할 수 있어야 한다.
- 알고리즘 구현 파일은 가능한 한 식과 흐름에 집중하고, 튜닝 기본값은 `tuning.rs`에 모은다.

## 그룹

- `normalization`
  - land / ocean cut, 거리 정규화, coast factor 거리 스케일
- `continent`
  - 대륙 노이즈 스케일, warp, coast roughness, blend weight
- `ridge`
  - ridge warp, ridge scale, 능선 결속감
- `terrain`
  - macro elevation, ruggedness, basinness, mountain mass, pass 계산
- `climate`
  - temperature / humidity field와 가중치
- `hydrology`
  - river source, flow, lake, wetness / aridity / alpine / wetland 계산
- `weights`
  - thermal / moisture / form / cover / ecotone 해석 weight
- `resolver`
  - preview biome 분기 임계값
- `preview`
  - biome preview signed height와 주요 색 규칙

## 불변식

1. atlas 기본값은 `AtlasTuning::default()` 한 곳에서 정의한다.
2. generation / resolver / debug는 가능하면 숫자 literal 대신 tuning 값을 읽는다.
3. tuning은 prototype용 기본값이므로, 문서와 코드가 함께 움직여야 한다.
