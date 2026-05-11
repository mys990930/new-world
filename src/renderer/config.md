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

- time-of-day value from the world calendar or fixed preset
- sun direction, color, and intensity
- ambient color and intensity
- fog color, density, and height falloff
- sky / horizon colors and overcast amount
- weather strength and wetness blend values derived from chunk weather presentation
- climate tint, humidity, and temperature bias
- quarter-view readability boosts for top faces, side shadows, silhouettes, and saturation

## Notes

- `Low` keeps the sun visible but disables shadow-map rendering.
- `Medium` enables a smaller hard-sun shadow map.
- `High` enables a larger hard-sun shadow map.
- The current default quality preset is still `medium`, so the default app path now uses visible direct sun plus shadows.
- The default fixed environment preset is a warm sunset quarter-view setup with stronger top/side readability and only a very light amount of atmosphere.
- The app bridge drives live `RenderEnvironment` values from world calendar time-of-day plus player-focus chunk weather scalars. The default preset remains the fallback before app/gameplay sync.
- Chunk `cloud` lowers direct light and raises fog, `rain` lowers saturation/contrast and raises wetness/weather strength, `temperature` shifts color temperature, `moisture` feeds haze/humidity tint, and `Storm` adds dark cool grading.
