# weather

## Role

- Define chunk-scoped weather scalar rules for simulation-owned progression.
- Provide the source spec for textmode, gameplay, and renderer consumers before Rust implementation.
- Keep weather meaning in structured state, not console strings or renderer-only tuning.

## State

Each active or loaded chunk has a world-owned weather state:

```text
ChunkWeatherState {
  temperature: 0.0..=1.0,
  moisture: 0.0..=1.0,
  cloud: 0.0..=1.0,
  rain: 0.0..=1.0,
  kind: Clear | Cloudy | Rain | Snow | Storm,
  updated_at_tick
}
```

`temperature` means local air temperature normalized for simulation. `moisture` means local atmospheric/ground humidity pressure. `cloud` means sky coverage. `rain` means precipitation pressure or intensity; snow is derived from precipitation plus low temperature.

## Ownership

- `simulation` owns hourly weather update rules and kind threshold derivation.
- `world` owns `ChunkWeatherState` storage and chunk query/apply APIs.
- `renderer` consumes scalar weather plus time of day and never derives gameplay weather from shader-local state.
- `new-world-textmode` formats the same structured state for diagnostics only.

## Update Cadence

- Fixed ticks advance the calendar through `time.md`.
- Weather updates only on in-game hour boundaries.
- The first implementation should update only active/loaded chunks selected by ECS/app.
- Lazy catch-up for unloaded chunks may later replay hour steps or sample a coarser deterministic state.

## Hourly Update Inputs

For each chunk, the target scalar values derive from:

- biome base range from graph-first `GraphBiomeKind`
- current chunk graph/Voronoi `temperature` and `hydration`
- previous chunk weather state
- neighboring chunk weather states
- biome seasonal coefficients
- deterministic seed/calendar noise for small local variation

Draft blend:

```text
biome_target = biome_range_midpoint adjusted by graph temperature/hydration
season_target = biome_target + seasonal coefficient for current season
neighbor_target = average of adjacent loaded chunk weather states
next = lerp(previous, mix(season_target, neighbor_target, 0.20), 0.25)
```

Clamp every scalar to both `0.0..=1.0` and the biome's hard range. Desert-like biomes may keep `rain` and `moisture` near zero even when neighbors are wet.

## Biome Base Ranges

Ranges are normalized `min..max` values for `temperature / moisture / cloud / rain`.

| Biome | temperature | moisture | cloud | rain | Seasonal note |
| --- | --- | --- | --- | --- | --- |
| ShallowOcean | 0.36..0.72 | 0.70..1.00 | 0.35..0.80 | 0.20..0.70 | mild temperature swing, high moisture |
| DeepOcean | 0.32..0.68 | 0.75..1.00 | 0.35..0.85 | 0.25..0.75 | stable and humid |
| Lake | 0.30..0.70 | 0.65..1.00 | 0.30..0.78 | 0.18..0.68 | dampens heat and cold |
| Marsh | 0.28..0.68 | 0.70..1.00 | 0.38..0.85 | 0.25..0.75 | cloudy and wet, freezes in winter |
| Swamp | 0.42..0.78 | 0.72..1.00 | 0.40..0.88 | 0.25..0.78 | humid, summer storm prone |
| FloodedForest | 0.38..0.74 | 0.72..1.00 | 0.38..0.85 | 0.24..0.76 | humid canopy, slow drying |
| Mangrove | 0.66..0.94 | 0.75..1.00 | 0.35..0.82 | 0.25..0.78 | hot, small winter swing |
| EstuarineCoast | 0.46..0.78 | 0.68..1.00 | 0.38..0.86 | 0.24..0.76 | windy wet coast |
| LagoonCoast | 0.58..0.88 | 0.68..1.00 | 0.30..0.78 | 0.18..0.68 | warm and humid |
| RockyCoast | 0.28..0.68 | 0.45..0.85 | 0.35..0.82 | 0.18..0.68 | cool marine cloud |
| SandyCoast | 0.46..0.82 | 0.38..0.78 | 0.22..0.68 | 0.08..0.52 | drier beach edge |
| Desert | 0.78..1.00 | 0.00..0.08 | 0.00..0.12 | 0.00..0.02 | tiny seasonal moisture pulse |
| SemiDesert | 0.62..0.92 | 0.05..0.22 | 0.02..0.25 | 0.00..0.10 | brief wet season |
| Steppe | 0.32..0.72 | 0.12..0.38 | 0.10..0.45 | 0.02..0.25 | large seasonal temperature swing |
| DryShrubland | 0.48..0.82 | 0.10..0.35 | 0.08..0.42 | 0.02..0.22 | spring moisture, dry summer |
| MediterraneanShrubland | 0.46..0.80 | 0.18..0.48 | 0.12..0.50 | 0.04..0.32 | wetter winter, dry summer |
| PolarIce | 0.00..0.18 | 0.05..0.28 | 0.12..0.55 | 0.00..0.25 | precipitation presents as snow |
| PolarBarrens | 0.02..0.26 | 0.08..0.35 | 0.12..0.55 | 0.00..0.28 | dry cold, snow possible |
| Tundra | 0.08..0.36 | 0.20..0.55 | 0.18..0.65 | 0.04..0.38 | summer thaw moisture |
| SubalpineWoodland | 0.18..0.52 | 0.35..0.70 | 0.22..0.72 | 0.08..0.50 | frequent snow in cold seasons |
| AlpineMeadow | 0.12..0.46 | 0.28..0.62 | 0.18..0.68 | 0.06..0.45 | cold, fast weather shifts |
| BorealForest | 0.12..0.48 | 0.38..0.78 | 0.25..0.78 | 0.10..0.58 | long snowy winter |
| TropicalRainforest | 0.70..0.96 | 0.78..1.00 | 0.45..0.92 | 0.35..0.90 | persistently wet |
| MonsoonForest | 0.68..0.94 | 0.55..0.95 | 0.32..0.88 | 0.18..0.82 | strong wet/dry season |
| TropicalDryForest | 0.66..0.92 | 0.28..0.62 | 0.18..0.62 | 0.06..0.42 | seasonal rain pulses |
| Savanna | 0.66..0.94 | 0.18..0.55 | 0.12..0.58 | 0.04..0.38 | wet season storms, dry season clear |
| TemperateRainforest | 0.32..0.68 | 0.72..1.00 | 0.42..0.90 | 0.28..0.82 | high cloud and rain |
| TemperateMixedForest | 0.28..0.70 | 0.42..0.78 | 0.24..0.72 | 0.10..0.56 | broad seasonal swing |
| TemperateBroadleafForest | 0.34..0.74 | 0.35..0.72 | 0.22..0.70 | 0.08..0.52 | spring/autumn wetter |
| TemperateGrassland | 0.30..0.76 | 0.18..0.48 | 0.12..0.55 | 0.04..0.34 | dry summer, storm fronts |

## Seasonal Coefficients

Coefficients are added to the biome target before clamping. They are small by default; biome ranges remain the hard guardrail.

| Biome group | Spring | Summer | Autumn | Winter |
| --- | --- | --- | --- | --- |
| desert/arid | temp +0.02, moist +0.02 | temp +0.05, moist -0.01 | temp -0.01, moist +0.01 | temp -0.04, cloud +0.01 |
| temperate forest/grass | moist +0.06, rain +0.04 | temp +0.06, rain -0.02 | moist +0.04, cloud +0.05 | temp -0.12, rain +0.03 |
| tropical/monsoon | rain +0.08 | temp +0.03, rain +0.10 | rain -0.04 | temp -0.02, rain -0.06 |
| boreal/alpine/tundra | moist +0.04 | temp +0.08, rain +0.02 | cloud +0.04 | temp -0.16, cloud +0.05 |
| ocean/coast/lake/wetland | moist +0.03 | temp +0.03 | cloud +0.04 | temp -0.06, cloud +0.03 |

## Weather Kind Thresholds

Derive `kind` after scalar update:

```text
Storm  if cloud >= 0.82 && rain >= 0.72
Snow   if rain >= 0.35 && temperature <= 0.18
Rain   if rain >= 0.40
Cloudy if cloud >= 0.38
Clear  otherwise
```

`Snow` is a precipitation presentation over cold chunks. Surface accumulation and thaw remain world-owned surface-condition rules.

## Renderer Contract

Renderer input should include chunk weather scalar state plus calendar/time-of-day:

- `cloud`: lowers direct light, softens ambient, increases fog/haze.
- `rain`: lowers saturation/contrast, increases wetness and precipitation strength.
- `temperature`: shifts color temperature warm/cool.
- `moisture`: feeds haze/fog and vegetation tint.
- `kind`: selects presentation families such as clear sky, rain particles, snow particles, and storm darkening.
- day/night lighting remains primary; weather modulates it rather than replacing it.

## Related Docs

- `simulation.md`
- `time.md`
- `../world/world.md`
- `../renderer/renderer.md`
- `../world/generation/biome/biome.md`
- `ecology_table.md`
