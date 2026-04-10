# noise

## Role

- Provide deterministic block-scale relief noise used by generation after atlas sampling.

## Responsibilities

- hash-based 2D noise
- fbm helpers
- ridged-fbm helpers
- shared smoothing/clamp/lerp utilities

## Notes

- Atlas controls macro landform layout.
- Generation noise adds sub-atlas relief so a large atlas cell does not collapse into a flat plateau or flat shelf in chunk space.
