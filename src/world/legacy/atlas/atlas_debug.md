# atlas_debug

## Role

- Rasterize atlas field maps and resolved preview maps into deterministic debug PNG outputs.

## Responsibilities

- Render scalar atlas fields such as landness, elevation, ridge, hydrology, temperature, humidity, and ecotone.
- Render categorical atlas previews such as overlay and biome preview.
- Keep atlas prototype outputs easy to compare across seeds and tuning changes.

## Default Outputs

- `00_landness.png`
- `01_elevation.png`
- `02_ridge.png`
- `03_hydrology.png`
- `04_temperature.png`
- `05_humidity.png`
- `06_overlay.png`
- `07_biome_preview.png`
- `08_ecotone.png`

## Biome Preview Rules

- `07_biome_preview.png` is still an atlas-cell preview, not a chunk or block-level render.
- Ocean uses a blue base and gets darker as the signed depth moves farther below sea level.
- Land biomes use their own palette and get darker as signed height rises farther above sea level.
- Coast stays pale yellow, desert stays orange, and polar terrain stays near white.
- River influence is applied as a tint on top of the biome color when the riverine signal is strong enough.
- Mountain ranges are now emphasized as an extra darkening pass on land biomes.
- That range shading is driven by `ridge_factor`, `mountain_mass`, `form.mountain`, and positive signed height.
- The result is that mountainous forest, grassland, steppe, or desert cells can still show a visible range silhouette instead of reading as a flat biome patch.

## Invariants

1. The same atlas input must always produce the same debug images.
2. Debug rendering must not redefine atlas ownership or classification rules.
3. Preview shading may improve readability, but it must not pretend to be final chunk realization.
