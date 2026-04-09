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
- reserved shadow quality selection

### `RenderEnvironment`

- fixed time-of-day value
- sun direction, color, and intensity
- ambient color and intensity
- fog color, density, and height falloff
- sky / horizon colors and overcast amount
- weather and wetness blend values
- climate tint, humidity, and temperature bias
- quarter-view readability boosts for top faces, side shadows, silhouettes, and saturation

## Inputs

- app bootstrap renderer configuration
- hardcoded renderer defaults for the current sunset prototype

## Outputs

- surface initialization policy
- camera defaults
- environment uniform source data
- fallback clear color when the app does not override it

## State Rules

- `RenderConfig` is immutable policy / default data once the renderer is created
- actual surface format / present mode selection stays in runtime state
- mutable time / weather / climate values belong to `RenderEnvironmentState`, not back in `RenderConfig`

## Invariants

- sample count must be greater than zero
- projection near / far planes must define a valid clip range
- environment values must stay finite
- weather blend values stay within `0..=1`
- `RenderConfig` does not own GPU resources

## Non-Responsibilities

- adapter capability queries
- surface configure calls
- shader / pipeline creation
- camera movement or gameplay camera rules

## Related Modules

- `surface.rs` consumes surface / environment policy
- `pipeline.rs` mirrors sample-count and rebuild metadata
- `camera.rs` consumes projection defaults
- `state.rs` holds the mutable environment copy used at runtime

## Notes

- The current default environment is a fixed sunset preset intended to approximate a Complementary-like warm quarter-view mood while staying cheap enough for low-end hardware.
- Low / medium / high quality presets are a policy layer only. Today they gate shader features through uniform flags; later they can also select shadow or post-process variants.
- `clear_color` remains as a fallback, but the current frame path prefers a sky / horizon blend derived from the active renderer environment.
