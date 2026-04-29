# cover

## Role

- define cover-phase and override rules for time-varying or hydrology-driven top surfaces

## Current Stub Rules

- `snowy_grass`
- `frozen_mud`
- `wet_season_greening`

## Notes

- these rules are intentionally small and composable
- later systems can add richer ecology and vegetation overlays on top
- the current surface-plan resolver uses them as thin block-selection nudges on top of material policy, not as a replacement for region ownership
