# hydrology/water_profile

## Role

- resolve the longitudinal water-profile anchor used by later hydrology terrain passes
- provide a shared local water surface, core floor target, and cut budget before visible water blocks are committed
- act as the pre-carve geometric reference for channel/floodplain shaping while leaving actual water emission to the post-carve `water_surface` pass
- expose the branch profile as an upper reference; later passes may lower the effective surface to match supported bed depth and bank containment

## Boundaries

- may derive anchor height and depth budget from carried branch anchors, slope, relief budget, and region incision style
- consumes a reach-style-adjusted local channel depth from the orchestration pass; it does not decide the reach style itself
- must treat the water surface as branch/profile-owned, not as a per-column terrain support value
- must not decide final terrain shape, gravel/point bars, voxel masks, or material blocks
- visible water placement happens later in `water_surface`

## Invariants

1. output heights must stay finite and inside hydrology height bounds
2. channel, floodplain, and bar passes should reference this profile instead of inventing separate water baselines
3. the profile is an anchor, not a guarantee that a visible water column will be emitted
4. a downstream profile sample must never rise above its upstream sample; if carried anchors conflict, the downstream anchor is clamped level or lower
5. local terrain height must not cap or bend the water-surface profile; terrain support is checked later
6. the target floor returned here may be higher for shallow upper/source reaches or deeper for broad lower reaches, but water still appears only if the later carved terrain can support the effective surface
