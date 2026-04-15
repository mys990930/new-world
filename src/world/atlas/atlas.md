# atlas

## ASCII Addendum

- The current atlas structure pass now performs a first explicit confluence snap solve after trunk and tributary branches are emitted.
- Chunk generation still owns the final carved river shape, but it can now sample confluence nodes as part of the atlas-owned drainage guide.
- Atlas remains intentionally macro at the current scale. The current plan is to add a separate deterministic meso layer for multi-chunk local terrain identity instead of shrinking atlas cells.
- That future meso layer should be derived from seed and nearby atlas context, generated on demand, and kept distinct from atlas biome-scale or mountain/drainage-scale ownership.

## 역할

- 청크 생성 이전 단계에서 atlas-scale 거시 환경 필드를 계산한다.
- 시드와 atlas 좌표만으로 재현 가능한 2D 환경 해석 결과를 제공한다.
- biome resolver 이전 단계의 climate / terrain / hydrology / cover 축을 보존한다.
- 큰 산맥과 배수망 같은 방향성 있는 거시 지형 구조를 청크 생성 이전 단계에서 소유한다.
- 디버그 검토용 PNG 출력 입력을 제공한다.

## 책임

- `AtlasCoord`, `AtlasArea`, atlas hard scale 정의
- structure region 단위와 on-demand 생성 규칙 정의
- deterministic seed sampling 규칙 정의
- ocean / continent / elevation / ridge / hydrology / climate field 계산
- mountain-chain spine / drainage path 같은 구조적 macro guide 계산
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

generate_atlas_structure(meta: &WorldMeta, area: AtlasArea) -> AtlasStructureMap
generate_atlas_structure_with_tuning(
    meta: &WorldMeta,
    area: AtlasArea,
    tuning: &AtlasTuning,
) -> AtlasStructureMap

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
6. 방향성 있는 큰 산맥과 강 흐름은 chunk 생성이 즉흥적으로 만들지 않고 atlas 구조를 기준으로 이어져야 한다.
7. atlas structure는 무한 월드 전체를 미리 생성하지 않고, `seed + structure region` 기준으로 필요할 때마다 재현 가능하게 생성되어야 한다.

## 하위 문서

- `seed.md`
- `scale.md`
- `tuning.md`
- `structure.md`
- `atlas_fields.md`
- `atlas_resolver.md`
- `atlas_debug.md`

## 현재 구현 메모

- atlas structure는 이제 region-owned mountain chain과 초기 drainage path graph를 on-demand로 함께 생성한다.
- 다만 chunk realization은 아직 structure graph를 직접 소비하지 않고 scalar-first hydrology를 유지한다.

- 첫 Rust prototype은 seed와 atlas 영역 크기를 입력으로 받아 PNG 여러 장을 생성하는 오프라인 실행기를 우선 제공한다.
- preview biome는 tuning을 위한 시각화 결과이며 authoritative biome contract가 아니다.
- atlas 기본 tuning 값은 `tuning.rs`에 모아두고, 구현 파일은 가능하면 그 값을 읽는 쪽으로 유지한다.
- 현재 청크 realization은 아직 scalar field 중심이지만, atlas 쪽에서는 이미 `structure.md` 기준의 산맥 spine과 초기 drainage path를 함께 소유하기 시작했다.
- structure graph는 전역 선계산이 아니라 on-demand region 생성과 캐시를 전제로 설계한다.
