# structure

## 역할

- atlas-scale에서 방향성 있는 거시 지형 골격을 정의한다.
- 청크 생성 이전 단계에서 큰 산맥의 spine, 분수계, 배수망, 강 경로를 결정한다.
- chunk 생성이 산맥과 강의 방향을 즉흥적으로 발명하지 않도록 atlas 쪽에서 장거리 연속성을 고정한다.

## 책임

- `AtlasStructureMap` 계약 정의
- 큰 산맥 / 작은 지맥의 chain 배치와 연결 규칙 정의
- 산맥 spine의 방향, 강도, 폭, branch 계층 정의
- pass / saddle / drainage divide 같은 구조적 전이점 정의
- headwater / trunk / tributary / sink를 포함한 drainage graph 정의
- atlas cell 경계를 넘는 river path 연속성 보장
- generation이 읽을 수 있는 structure window와 distance-field 입력 의미 정의

## 비책임

- 실제 block 채우기
- chunk-local smoothing
- 최종 sediment / top block 재질 선택
- water voxel 높이 양자화
- live world storage mutation

## 소유 구조

- `AtlasStructureMap`
- `MountainChainGraph`
- `MountainChainId`
- `MountainSpineSegment`
- `DrainageGraph`
- `DrainageNode`
- `RiverPathId`
- `RiverPathSegment`

## 공개 인터페이스

```rust
generate_atlas_structure(meta: &WorldMeta, area: AtlasArea) -> AtlasStructureMap
generate_atlas_structure_with_tuning(
    meta: &WorldMeta,
    area: AtlasArea,
    tuning: &AtlasTuning,
) -> AtlasStructureMap
```

## 현재 스캐폴드 계약

- `AtlasStructureMap`은 현재 `AtlasArea`와 비어 있을 수 있는 graph 컨테이너를 소유한다.
- `MountainChainGraph`는 `MountainSpineSegment` 목록을 소유한다.
- `DrainageGraph`는 `DrainageNode`와 `RiverPathSegment` 목록을 소유한다.
- 첫 단계 구현에서는 생성 함수가 area-stable empty graph를 반환해도 괜찮다.
- 다음 단계부터 mountain chain과 drainage segment를 채워 넣는다.

## generation이 읽는 방식

1. generation은 target chunk 주변의 atlas structure window를 요청한다.
2. chunk 내부에서는 nearby spine/path segment를 `xz` 평면으로 rasterize한다.
3. 그 결과로 `distance-to-ridge`, `along-ridge`, `distance-to-channel`, `along-channel` 같은 chunk-local guide를 만든다.
4. ridge, valley, floodplain, wetted channel은 이 guide를 바탕으로 현실화한다.
5. local noise는 구조를 뒤집지 않고 표면 디테일만 추가한다.

## 불변식

1. `(seed, generator_version, area)`가 같으면 같은 구조가 나와야 한다.
2. 산맥과 강의 macro direction은 chunk 생성 순서와 무관해야 한다.
3. 하나의 산맥 chain은 복수 atlas cell에 걸쳐도 일관된 heading과 branch 계층을 유지해야 한다.
4. 강 경로는 가능한 한 상류에서 하류로 연결된 path로 표현되어야 한다.
5. headwater는 고지대 spine, pass, upland divide, basin outlet 같은 구조적 원인과 연결되어야 한다.
6. generation은 atlas 구조를 세밀하게 변형할 수는 있어도, 전혀 다른 거시 방향을 새로 발명해서는 안 된다.

## 단계적 도입 방향

1. 기존 scalar ridge/mountain field 위에 `MountainChainGraph`를 먼저 도입한다.
2. 그 위에서 `DrainageGraph`와 `RiverPathSegment`를 도입한다.
3. river/lake scalar field 일부를 structure-derived field로 교체한다.
4. generation이 path-distance 기반 ridge/river realization으로 넘어간다.

## 현재 구현 메모

- 현재 코드는 아직 scalar field 중심이다.
- 이 문서는 다음 revision에서 atlas가 소유해야 할 구조 계약을 먼저 고정한다.
