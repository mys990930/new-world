# atlas_fields

## 역할

- atlas raw field와 derived factor를 계산한다.

## 책임

- land / ocean mask
- continent id / continent core factor
- macro elevation / slope / ruggedness / ridge / mountain mass
- basin / river / lake / river distance
- temperature / humidity / inlandness
- aridity / wetness / polar / alpine / ecotone factor
- climate / moisture / form / overlay / cover weights

## 처리 순서

1. landness와 continent structure 계산
2. ocean/coast distance와 continent metadata 계산
3. elevation / ridge / mountain structure 계산
4. drainage / river / lake field 계산
5. temperature / humidity / inlandness 계산
6. derived factor와 axis weight 계산

## 불변식

1. scalar field는 명시적으로 정규화된 범위를 가져야 한다.
2. hydrology는 ocean/land 구조와 모순되면 안 된다.
3. preview tuning 단계에서도 field 이름과 의미는 문서 기준으로 유지한다.
4. 기본 tuning 값은 `tuning.rs`에서 읽고, 계산식은 그 값을 조합해 사용한다.
