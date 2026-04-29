# domain

## Role

- resolve the coherent categorical material domain for one surface column before block policy lookup
- preserve the sampled hard region owner while choosing whether a visible foreign region owner is locally justified
- preserve the sampled hard region owner for gameplay/query while letting visible foreign material domains compete through support-driven scoring
- keep material transitions as supported displaced boundaries rather than dithered per-column noise

## Responsibilities

- classify `RegionClassCell` values into broad `MaterialDomainKind` categories
- consume hard owner, `RegionClassInfluenceSet`, `SmoothedColumn` material-support axes,
  `HydrologyColumn`, and world x/z
- emit `MaterialDomainSample` with:
  - hard owner and hard material domain
  - chosen visible owner and visible material domain
  - candidate owner/domain when a foreign domain was considered
  - transition strength, deterministic phase, boundary displacement, support, and reason
- reject warp-only foreign owners when no broad generation material support, hydrology, or coastal evidence supports crossing the hard owner boundary
- pick the visible foreign-domain candidate from the influence neighborhood by support-driven scoring, not by raw foreign influence weight alone
- accept foreign owners only when a low-frequency boundary displacement is backed by broad support such as material wetness/exposure/sediment/soil axes, channels, standing water, saturation, gravel bars, or shoreline evidence; local slope and concavity alone must not authorize noisy cover changes
- allow rocky-coast to green/dry-grassland crossings on connected upper terraces only when the terrain is above sea level, gentle, and locally stable
- treat generation-derived wetness, exposure, sediment, and soil/cover support as the common
  transition grammar instead of requiring pairwise transition rules for every terrain combination

## Non-Responsibilities

- mapping `RegionArchetype` to `MaterialPolicyId`
- choosing top/filler/core block keys
- solving atlas classification, meso placement, terrain height, hydrology, or final voxel writes
- hiding artifacts with salt-and-pepper dither, per-column speckle, or visual noise inside domain interiors

## Public Surface

```rust
material_domain_kind_for_region(region: RegionClassCell) -> MaterialDomainKind

sample_material_domain(input: MaterialDomainInput<'_>) -> MaterialDomainSample
```

## Boundary Contract

- `hard_owner` remains the gameplay/query owner even when `visible_owner` is accepted for material expression.
- `visible_owner` may cross the hard owner only when the foreign domain is supported by broad generation material axes, hydrology, or coastal evidence.
- local support is evaluated before atlas-neighborhood foreign-owner competition, so final material
  can follow the continuous generation support field instead of using atlas influence as the
  primary boundary generator.
- local support may select a visible material domain even when the atlas influence neighborhood
  does not expose that domain as a foreign owner; wetland/exposed/cover support should then come
  from the continuous generation support field rather than from a direct atlas-cell boundary.
- if a local support domain wins without a matching foreign atlas owner, the hard owner remains the
  visible owner for identity while the visible material domain changes; this keeps ownership stable
  without forcing block material to remain on atlas-owner lines.
- among foreign domains present in the influence neighborhood, the resolver should consider the best supported candidate rather than blindly favoring whichever foreign domain has the highest raw influence weight.
- the support test compares whether the candidate visible domain is better supported than the hard
  domain by the generation material-support axes and final hydrology.
- Boundary displacement can move an already supported boundary, but it cannot by itself authorize a large smooth foreign-owner curve.
- Domain-local displacement is a small tie-breaker after broad support and atlas-edge influence,
  not a second broad owner field; it must not introduce a new large curve over the region boundary.
- Domain displacement may contain subchunk value-noise components sampled at block columns, but it
  must not use independent hash speckles that create isolated visible-domain spots away from the
  edge.
- Each sampled column still resolves to one categorical visible domain; interiors must not be mottled with dither.
- The phase is deterministic from world x/z and the two categorical domains, so adjacent samples should form connected runs rather than salt-and-pepper alternation.
- Material policy mapping remains in `resolve.rs`; this module only reasons about `RegionArchetype` and region-category data.
- `resolve.rs` may still perform hydrology/coast-owned block-stack overrides after this module chooses
  a visible owner, but broad material support should primarily move the domain boundary. It must not
  become a local slope/height cover-noise source inside a single visible environment.

## Current Integration

- The module is exported from `surface/mod.rs`.
- `resolve.rs` consumes `sample_material_domain(...)` before material policy lookup so visible material ownership no longer has to be identical to raw atlas-cell ownership.
- `resolve.rs` still owns final policy mapping and hydrology/coast-owned block-stack subdivision inside one accepted material domain.
