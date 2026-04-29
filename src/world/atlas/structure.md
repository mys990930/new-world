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
- `AtlasStructureRegionCoord`
- `AtlasStructureRegion`
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

atlas_structure_region_coord_for_atlas(coord: AtlasCoord) -> AtlasStructureRegionCoord
atlas_structure_regions_covering_area(area: AtlasArea) -> Vec<AtlasStructureRegionCoord>
```

## 현재 스캐폴드 계약

- current drainage scaffold now treats each spawned major drainage basin as having one primary trunk rather than emitting mirrored equal-status trunks from both sides of the same mountain chain anchor.
- current drainage nodes now include an explicit confluence-solving pass that snaps tributary outlets onto nearby downstream segments inside the requested area.
- after confluence snapping, a non-crossing cleanup pass now truncates subordinate branches at the first detected junction so launch-scale rivers merge instead of crossing through each other.
- drainage ownership is now solved on the expanded structure sample before the result is cropped back to the requested area, so river segments carry deterministic ownership metadata derived from a wider local tree instead of from the strict visible crop alone.
- each `RiverPathSegment` now carries `basin_id`, `main_stem_river_id`, and `parent_river_id`.
- `main_stem_river_id` currently means the first visible trunk ancestor inside the sampled structure window, not a global world-basin polygon id.
- `parent_river_id` is optional and only guaranteed when the immediate confluence or merge target was visible inside the sampled structure window.
- pass-outlet and sink solving still remain later steps.
- river path segments now also carry downstream progress ranges so chunk generation can recover branch progress from the nearest segment projection.
- this structure graph should stay macro and directional. Smaller multi-chunk terrain identity such as hill groups, cliff bands, or local basins is expected to come from a later meso layer rather than from overloading mountain/drainage segments.
- meso is therefore planned as a sibling atlas layer, not as an extension of `AtlasStructureMap`.

- `AtlasStructureMap`은 현재 `AtlasArea`와 비어 있을 수 있는 graph 컨테이너를 소유한다.
- `AtlasStructureRegionCoord`는 atlas structure cache/ownership의 기본 단위 좌표다.
- `AtlasStructureRegion`은 region의 core area와 padded area를 계산한다.
- `MountainChainGraph`는 `MountainSpineSegment` 목록을 소유한다.
- `DrainageGraph`는 `DrainageNode`와 `RiverPathSegment` 목록을 소유한다.
- structure 생성은 requested area만 보지 않고, chain reach를 고려해 area 주변 region까지 함께 샘플한 뒤 결과를 requested area로 crop한다.
- owner region은 "그 chain/path의 anchor를 소유한 core region"으로 정의한다.

## generation이 읽는 방식

1. generation은 target chunk 주변의 atlas structure window를 요청한다.
2. chunk 내부에서는 nearby spine/path segment를 `xz` 평면으로 rasterize한다.
3. 그 결과로 `distance-to-ridge`, `along-ridge`, `distance-to-channel`, `along-channel` 같은 chunk-local guide를 만든다.
4. ridge, valley, floodplain, wetted channel은 이 guide를 바탕으로 현실화한다.
5. local noise는 구조를 뒤집지 않고 표면 디테일만 추가한다.

## Visible Guide Suppression

- mountain spine segments and river path segments are structural guides, not final visible polylines.
- generation may preserve their long-range direction, ownership, and downstream continuity, but the final terrain must not reveal raw segment chords as straight cuts or repeated smooth curves.
- chunk stages should convert skeleton geometry into broad envelopes first, then use world-space warping, branch-stable variation, confluence-aware blending, shoulders, benches, and material deposition to make the visible form terrain-like.
- confluence snapping and crossing cleanup are topology constraints. Their snapped points must be softened downstream so they do not become obvious angular artifacts.

## Structure Region Unit

- atlas structure의 순차 생성 단위는 chunk가 아니라 `structure region`이다.
- 기본 region 크기는 `8 x 8 atlas cells`다.
- region은 cache와 ownership의 기본 단위이고, requested area는 하나 이상의 region으로 덮여 해석된다.
- 각 region은 core area를 가지며, 필요할 때 padded area를 함께 사용한다.

## On-Demand Generation Rule

1. 런타임은 월드 전체 mountain/drainage graph를 미리 생성하지 않는다.
2. `generate_atlas_structure(area)`는 requested area를 structure reach만큼 확장한 뒤, 그 expanded area를 덮는 region만 생성한다.
3. 각 region은 자기 core 안의 deterministic anchor seed만 소유한다.
4. major/minor chain은 그 anchor에서 시작해 global heading field를 따라 marching한다.
5. 이렇게 생성된 segment 중 requested area를 건드리는 것만 `AtlasStructureMap`에 남긴다.

## Ownership Rule

- 하나의 `MountainSpineSegment`와 `RiverPathSegment`는 `owner_region`을 가진다.
- `owner_region`은 그 segment가 속한 chain/path의 anchor가 처음 배치된 core region이다.
- segment는 owner region 밖으로 뻗을 수 있지만, 같은 seed라면 어떤 순서로 region을 생성해도 owner와 shape가 바뀌면 안 된다.
- region 경계를 넘는 continuity는 "경계에서 연결해주는 후처리"가 아니라, anchor와 heading field가 deterministic하다는 사실에서 나와야 한다.

## 불변식

1. `(seed, generator_version, area)`가 같으면 같은 구조가 나와야 한다.
2. 산맥과 강의 macro direction은 chunk 생성 순서와 무관해야 한다.
3. 하나의 산맥 chain은 복수 atlas cell에 걸쳐도 일관된 heading과 branch 계층을 유지해야 한다.
4. 강 경로는 가능한 한 상류에서 하류로 연결된 path로 표현되어야 한다.
5. headwater는 고지대 spine, pass, upland divide, basin outlet 같은 구조적 원인과 연결되어야 한다.
6. generation은 atlas 구조를 세밀하게 변형할 수는 있어도, 전혀 다른 거시 방향을 새로 발명해서는 안 된다.
7. requested area 바깥 anchor가 만든 chain도 requested area를 통과할 수 있으므로, region cover는 requested area와 같은 크기로만 잘라 계산하면 안 된다.

## 단계적 도입 방향

1. region-based ownership과 `MountainChainGraph`를 먼저 도입한다.
2. 그 위에서 `DrainageGraph`와 `RiverPathSegment`를 도입한다.
3. river/lake scalar field 일부를 structure-derived field로 교체한다.
4. generation이 path-distance 기반 ridge/river realization으로 넘어간다.

## 현재 구현 메모

- 현재 코드는 region-based deterministic mountain chain 위에서 초기 drainage graph까지 함께 생성하는 단계다.
- drainage는 launch scope에서 basin당 하나의 주 trunk를 우선 만들고, tributary는 그 trunk나 더 강한 기존 branch에 붙도록 정리한다.
- downstream progress는 이제 generation이 river stage와 water-surface bias에 직접 사용할 수 있도록 segment에 기록된다.

- distributary/delta exception은 아직 구조 단계에 도입하지 않았다. 현재 inland launch scope에서는 큰 줄기는 합류할 수는 있어도 서로 교차하거나 다시 분기하지 않는 tree-like drainage를 목표로 한다.
