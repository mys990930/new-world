# hydrology/water_profile

## Role

- resolve the longitudinal water-profile anchor used by later hydrology terrain passes
- provide a shared local water surface, core floor target, and cut budget before visible water blocks are committed

## Boundaries

- may derive anchor height and depth budget from carried branch anchors, slope, relief budget, and region incision style
- must not decide final terrain shape, gravel/point bars, voxel masks, or material blocks
- visible water placement happens later in `water_surface`

## Invariants

1. output heights must stay finite and inside hydrology height bounds
2. channel, floodplain, and bar passes should reference this profile instead of inventing separate water baselines
3. the profile is an anchor, not a guarantee that a visible water column will be emitted
