# meso_preview

## Role

- render a dedicated meso-only preview on top of a flat plain baseline
- let us inspect resolved multi-chunk meso shape before it is blended back into the real prototype terrain
- reuse the same runtime meso-apply path as chunk generation, so preview tuning stays relevant to the actual generator

## Inputs

- positional `seed`
- optional chunk center via `--center-x <i32>` and `--center-z <i32>`
- optional footprint via `--radius <i32>`
- optional sampling density via `--blocks-per-pixel <u32>`
- optional feature isolation via `--feature <all|hill_cluster|shallow_basin|escarpment_band|upland_terrace|ravine|coastal_cliff_band|dune_field|crater>`
- optional corridor mode via `--corridors <none|live>`
- optional overlay via `--overlay <none|hill_peaks>`
- optional flat baseline controls via:
  - `--base-height <f32>`
  - `--relief-budget <f32>`
  - `--contour-step <f32>`
- optional `--output <path>`

## Outputs

- a top-down PNG under `target/meso-preview/` by default
  - the image includes chunk grid lines plus a top/left coordinate frame with chunk `x/z` labels and center-chunk highlight
- stdout diagnostics for:
  - center chunk region identity
  - original vs filtered meso guide sample
  - per-preview delta range and occupancy
  - optional hill-peak candidate count when `--overlay hill_peaks` is enabled
  - applied feature keys on the center chunk

## Preview Model

1. Build the normal chunk generation scaffold for each preview chunk.
2. Clone the chunk's `MesoGuideMap`.
3. If a single feature was requested, zero the non-target guide channels on that cloned guide map.
4. Replace the normal base prototype with a flat plain prototype using the requested `base_height` and `relief_budget`.
5. Run `build_chunk_meso_applied_prototype_for_feature(...)` with an optional exclusive feature filter.
6. Render the resulting `height - base_height` field as a meso-only top-down heatmap with hillshade, contours, chunk grid lines, and explicit chunk-coordinate reference labels.
7. If `--overlay hill_peaks` is requested, draw the filtered hill-cluster local-peak candidates from the same guide map on top of that heatmap so candidate density can be inspected before owner-region sparsening and per-hill resolve.

## Why Flat-Base Preview Exists

- it isolates the meso landform itself from macro heightfield noise
- it makes multi-chunk ownership, overlap, contour shape, and seam behavior much easier to read
- it gives us a stable tuning target before we re-blend the resolved meso surface into the real prototype heightfield

## Candidate Workflow

- `meso_preview` does not search for promising terrain on its own
- use `terrain_find` first when you want strong candidates:

```bash
cargo run --bin terrain_find -- 42 --meso hill_cluster --top 5
```

- then inspect a chosen candidate directly in the isolated preview:

```bash
cargo run --bin meso_preview -- 42 --center-x -57 --center-z 93 --radius 10 --feature hill_cluster
```

Inspect a ravine candidate on the same flat baseline:

```bash
cargo run --bin meso_preview -- 42 --center-x 40 --center-z -29 --radius 0 --feature ravine
```

## Examples

Inspect only hill clusters on a flat baseline:

```bash
cargo run --bin meso_preview -- 42 --center-x -57 --center-z 93 --radius 10 --feature hill_cluster
```

Inspect hill clusters with local peak candidates overlaid:

```bash
cargo run --bin meso_preview -- 42 --center-x -57 --center-z 93 --radius 10 --feature hill_cluster --overlay hill_peaks
```

Inspect all currently runtime-backed meso channels together:

```bash
cargo run --bin meso_preview -- 42 --center-x -57 --center-z 93 --radius 10 --feature all
```

Keep live corridor attenuation instead of stripping it out:

```bash
cargo run --bin meso_preview -- 42 --center-x -57 --center-z 93 --radius 10 --feature hill_cluster --corridors live
```

Preview a taller plain baseline with coarser sampling:

```bash
cargo run --bin meso_preview -- 42 --center-x -57 --center-z 93 --radius 10 --feature hill_cluster --base-height 56 --relief-budget 32 --blocks-per-pixel 2
```
