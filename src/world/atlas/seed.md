# seed

## 역할

- world seed에서 atlas용 deterministic sampling seed를 파생한다.
- atlas generation이 순서 독립적으로 같은 결과를 내도록 좌표 기반 샘플링 규칙을 제공한다.

## 책임

- hash / value noise / fbm / ridged-fbm 같은 순수 샘플링 유틸 제공
- salt 기반 field 분리
- atlas prototype 전역에서 재사용되는 noise helper 유지

## 불변식

1. 같은 `(seed, salt, coord)` 입력은 항상 같은 샘플 값을 반환한다.
2. 순회 순서나 병렬화 여부에 따라 결과가 달라지면 안 된다.
3. atlas seed helper는 내부 mutable RNG state를 소유하지 않는다.

## 메모

- 첫 구현은 외부 noise crate 없이 최소 deterministic sampler를 직접 제공한다.
- 필요하면 이후 성능/품질 검증 뒤에 구현만 교체할 수 있다.
