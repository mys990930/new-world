# atlas

## 역할

- 청크 생성 이전 단계에서 atlas-scale 거시 환경 필드를 계산한다.
- 시드와 atlas 좌표만으로 재현 가능한 2D 환경 해석 결과를 제공한다.
- biome resolver 이전 단계의 climate / terrain / hydrology / cover 축을 보존한다.
- 디버그 검토용 PNG 출력 입력을 제공한다.

## 책임

- `AtlasCoord`, `AtlasArea`, atlas hard scale 정의
- deterministic seed sampling 규칙 정의
- ocean / continent / elevation / ridge / hydrology / climate field 계산
- thermal / moisture / form / overlay weight 계산
- preview biome 해석 결과 제공
- atlas debug image rasterization 입력 제공

## 비책임

- 실제 block 채우기
- live world storage mutation
- async worker orchestration
- renderer 업로드
- authoritative biome/material 확정

## 공개 인터페이스

```rust
generate_atlas_fields(meta: &WorldMeta, area: AtlasArea) -> AtlasFieldMap
generate_atlas_fields_with_tuning(
    meta: &WorldMeta,
    area: AtlasArea,
    tuning: &AtlasTuning,
) -> AtlasFieldMap

resolve_atlas(fields: &AtlasFieldMap) -> AtlasResolvedMap
resolve_atlas_with_tuning(
    fields: &AtlasFieldMap,
    tuning: &AtlasTuning,
) -> AtlasResolvedMap

write_debug_images(
    fields: &AtlasFieldMap,
    resolved: &AtlasResolvedMap,
    output_dir: impl AsRef<Path>,
) -> Result<Vec<PathBuf>, AtlasDebugError>
```

## 불변식

1. atlas 결과는 `(seed, generator_version, area)`가 같으면 동일해야 한다.
2. atlas는 실제 block 데이터를 소유하지 않는다.
3. atlas는 final biome name보다 environment axis 보존을 우선한다.
4. atlas debug 출력은 tuning에 필요한 복수의 2D 맵을 제공해야 한다.
5. atlas prototype은 chunk realization과 분리된 오프라인 검증 경로를 유지한다.

## 하위 문서

- `seed.md`
- `scale.md`
- `tuning.md`
- `atlas_fields.md`
- `atlas_resolver.md`
- `atlas_debug.md`

## 현재 구현 메모

- 첫 Rust prototype은 seed와 atlas 영역 크기를 입력으로 받아 PNG 여러 장을 생성하는 오프라인 실행기를 우선 제공한다.
- preview biome는 tuning을 위한 시각화 결과이며 authoritative biome contract가 아니다.
- atlas 기본 tuning 값은 `tuning.rs`에 모아두고, 구현 파일은 가능하면 그 값을 읽는 쪽으로 유지한다.
