# hydrology/bars

## Role

- shape depositional gravel bars and point bars outside the wetted channel
- move eligible bank-side terrain toward a near-water freeboard instead of treating bars as a material-only mask
- build an actual low, bank-adjacent bench that can quantize to the same visible block height as nearby water, so it reads as a creekside gravel flat rather than a high rocky terrace
- keep bars continuous along the same depositional water-height band unless the local branch curvature meaningfully changes the favored bank side

## Boundaries

- owns bar strength and bar terrain-height adjustment
- may use water-profile anchors, expected visible-water depth, channel width, confluence, widening, and inside-bend signals
- may use a reach-continuity signal from local height-band fit, slope/grade, and lower-reach style so bars do not blink on/off at segment-local event boundaries
- does not decide final water columns or block ids; it emits terrain and `gravel_bar_strength` for later policy

## Invariants

1. bars should stay outside the core wetted ribbon
2. bars should read as low bank-adjacent flats before terrain climbs into hillside or valley wall
3. bar output must remain normalized and finite for voxel/material consumption
4. bar terrain should reference the expected depositional water surface, not a high longitudinal anchor that can turn the bar into a terrace
5. weak-curvature reaches should preserve bar continuity across adjacent columns and chunks; strong curvature may move or concentrate the bar toward the inside bend
