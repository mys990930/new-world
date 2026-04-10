# config

## Role

- Define renderer configuration data separately from live GPU state
- Keep quality presets and fixed-environment defaults out of the frame loop code

## Owned Data

### `RenderConfig`

- preferred present mode
- preferred surface format policy
- depth format
- fallback clear color
- sample count
- upload budget / staging policy
- debug rendering toggles
- camera projection defaults
- render quality preset
- fixed renderer environment values

### `RenderQualityConfig`

- quality tier (`Low`, `Medium`, `High`)
- fog enable flag
- color grading enable flag
- climate tint enable flag
- weather tint enable flag
- shadow quality policy

### `RenderEnvironment`

- fixed time-of-day value
- sun direction, color, and intensity
- ambient color and intensity
- fog color, density, and height falloff
- sky / horizon colors and overcast amount
- weather and wetness blend values
- climate tint, humidity, and temperature bias
- quarter-view readability boosts for top faces, side shadows, silhouettes, and saturation

## Notes

- `Low` keeps the sun visible but disables shadow-map rendering.
- `Medium` enables a smaller hard-sun shadow map.
- `High` enables a larger hard-sun shadow map.
- The current default quality preset is still `medium`, so the default app path now uses visible direct sun plus shadows.
