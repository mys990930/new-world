# macro_map_preview

## Role

- Render a deterministic top-down PNG composite for the graph-field-resolved macro map stage.
- Show separated ocean, island/coast, inland elevation, peak whitening, ridge/fault/coast candidate
  edge overlays, and stage 6 selected hydrology overlays in one image.
- Keep default filenames short while preserving detailed settings in PNG metadata.

## Inputs

- positional: `<seed> <center-x> <center-z>`
  - `center-x` and `center-z` are world-block coordinates.
- optional:
  - `--width <u32>`
  - `--height <u32>`
  - `--world-span-blocks <i32>`
  - `--region-size-blocks <i32>`
  - `--site-spacing-blocks <i32>`
  - `--land-bias <f32>`
  - `--stage macro_map`
  - `--output <path>`

## Defaults

- `--width 3840`
- `--height 2160`
- `--world-span-blocks 32768`
- `--region-size-blocks DEFAULT_GRAPH_REGION_SIZE_BLOCKS`
- `--site-spacing-blocks DEFAULT_SITE_SPACING_BLOCKS`
- `--land-bias MacroMapConfig::new(...).land_bias`
- `--stage macro_map`
- output: `target/macro-map-preview/s<seed>_x<center-x>_z<center-z>.png`

## Outputs

- One RGB PNG composite:
  - ocean and lake areas use blue ranges.
  - coast areas use sandy transition colors.
  - inland land and island components use green-to-upland colors.
  - high elevation approaches white, and peak colors are white.
  - all graph Voronoi edges are drawn as a faint base overlay from actual corner-to-corner graph geometry.
  - ridge candidate edges are white overlays.
  - fault candidate edges are red overlays.
  - coast candidate edges are sandy overlays.
  - selected river chains are drawn from hydrology `GraphRiverSegment` results as cyan/blue
    corner-to-corner edge chains.
  - river width and opacity follow the hydrology segment's selected/display `flow_accumulation`,
    not `raw_flow_accumulation`; lake terminal discharge caps are therefore visible in the preview.
  - lake terminal/inlet selection is already reduced by hydrology before rendering: per-lake
    top-N chain limits, short visible inlet segments, and lake-area discharge caps all affect
    the displayed segment set.
  - lake, lake inlet, lake outlet, sink, and coast outlet drainage nodes are marked with small
    overlay dots. Lake inlets and outlets use distinct colors so lake contact topology can be
    checked without reading raw dumps.
- A compact in-image legend with an elevation color bar and overlay keys.
- A PNG iTXt chunk named `new-world-preview-header` containing seed, generator version, stage,
  center, dimensions, world span, graph region sizing, land/ocean tuning values, graph area, site
  count, candidate edge count, coast/ridge/fault edge counts, selected river/lake/sink/outlet counts,
  lake/ocean terminal segment counts, lake component count, inland water site count, ocean component
  count, lake-capped segment count, lake inlet/outlet count, invalid lake contact/intersection
  count, lake/ocean max display/raw flow, sea level, and source notes.

## Output Path Rules

- With no `--output`, the binary writes `target/macro-map-preview/s<seed>_x<center-x>_z<center-z>.png`.
- With `--output <path>.png`, the binary writes that exact PNG path.
- With `--output <directory>`, the binary writes `s<seed>_x<center-x>_z<center-z>.png` below that directory.
- Width, height, span, spacing, stage, and generator version stay in PNG metadata rather than default filenames.

## Current Flow

1. Parse required seed and world-block center.
2. Resolve the preview window from image dimensions and `--world-span-blocks`.
3. Build a padded Delaunay/circumcenter Voronoi dual graph patch through `generate_voronoi_graph_patch(...)`.
4. Build the macro map through `generate_macro_map(&patch, MacroMapConfig::new(...))`, overriding
   `land_bias` from CLI options when provided.
5. Solve selected hydrology through `solve_hydrology(&patch, &macro_map, HydrologyConfig::default())`.
6. Generate the RGB pixel buffer with Rayon.
7. Draw a faint base Voronoi edge overlay by resolving each graph edge's corners against the graph
   patch's `VoronoiCorner.position` values, clipping the world-space segment to the preview window,
   and projecting it onto pixel centers.
8. Draw candidate edge overlays by resolving `MacroEdge.corners` against the graph patch's
   `VoronoiCorner.position` values, clipping the world-space segment to the preview window, and
   projecting it onto pixel centers. This layer draws graph-derived coast, ridge, and fault guide overlays.
9. Draw selected river chains and lake/inlet/outlet/sink/coast-outlet drainage node markers from
   hydrology results.
10. Draw the compact legend and encode PNG metadata.

The fill layer is a nearest-site diagnostic color field. Its apparent pixel boundary can differ from
the rendered edge overlay because the overlay is not inferred from nearest-site color changes; it uses
the graph's explicit `edge.corners` and Delaunay triangle circumcenter `VoronoiCorner.position`
segment. `graph_voronoi_preview` identity mode darkens nearest-site distance ties and also overlays
the explicit topology, while `macro_map_preview` exposes the actual graph edge network underneath the
highlighted coast/ridge/fault guides.

## Integration Note

The binary uses the public macro map API exposed by `world::generation::macro_map`:

```rust
new_world::world::generation::generate_macro_map(&patch, MacroMapConfig::new(seed, generator_version))
new_world::world::generation::solve_hydrology(&patch, &macro_map, HydrologyConfig::default())
```

Goal contract: this preview should show how graph base `continentality/elevation_seed` resolves into
continent/ocean/island ownership, macro elevation, coast, ridge/fault guide, and selected hydrology.
The graph continentality and elevation maps should visibly match the macro map's land/ocean and
high/low patterns, and selected rivers should follow continuous downstream chains to ocean/coast or
explicit lake/sink resolution. Lake-bound selected rivers should terminate at `LakeInlet` vertices,
and lake outlet rivers should start from separated `LakeOutlet` vertices rather than skimming along
lake edges.

If a future worker adds a preview-specific request type, keep this CLI and output path contract stable and replace only the internal map construction.

## Example

```bash
cargo run --bin macro_map_preview -- 42 0 0
```

Lower-resolution smoke check:

```bash
cargo run --bin macro_map_preview -- 42 0 0 --width 640 --height 360 --output target/macro-map-preview/smoke.png
```
