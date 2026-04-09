# atlas_resolver

## 역할

- atlas field를 읽어 dominant thermal / moisture / form / overlay와 preview biome를 해석한다.

## 책임

- dominant class 선택
- preview biome classification
- tuning용 categorical debug surface 제공

## 비책임

- authoritative biome table 소유
- chunk material rule 확정
- vegetation placement rule 확정

## 불변식

1. preview biome는 atlas tuning을 위한 읽기 쉬운 시각화 결과다.
2. atlas field가 유지하는 연속 weight 정보는 resolver 이후에도 보존된다.
3. ocean / coast / alpine / wetland 같은 overlay 성격은 biome preview에서 우선 반영될 수 있다.
