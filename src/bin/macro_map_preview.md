# macro_map_preview

## Role

- Render a deterministic top-down PNG composite for the graph-field-resolved macro map and
  canonical noisy Voronoi edge stages.
- Show separated ocean, island/coast, inland elevation, peak whitening, ridge/fault/coast candidate
  edge overlays, and stage 6 selected hydrology overlays in one image.
- Keep default filenames short while preserving detailed settings in PNG metadata.

## Inputs

- positional: `<seed> <cx> <cz> <r>`
  - `cx` and `cz` are chunk coordinates.
  - `r` is an inclusive square chunk radius; the horizontal footprint is `(2r + 1)` chunks.
- optional:
  - `--width <u32>`
  - `--height <u32>`
  - `--region-size-blocks <i32>`
  - `--site-spacing-blocks <i32>`
  - `--land-bias <f32>`
  - `--stage macro_map`
  - `--output <path>`

## Defaults

- `--width 3840`
- `--height 2160`
- `--region-size-blocks DEFAULT_GRAPH_REGION_SIZE_BLOCKS`
- `--site-spacing-blocks DEFAULT_SITE_SPACING_BLOCKS`
- `--land-bias MacroMapConfig::new(...).land_bias`
- `--stage macro_map`
- output: `target/macro-map-preview/s<seed>_cx<cx>_cz<cz>_r<r>.png`
- The horizontal world span is `(2r + 1) * CHUNK_EDGE` blocks. Non-square image sizes keep the
  same world-blocks-per-pixel scale on both axes, so the visible Z span follows the image aspect;
  use square `--width`/`--height` for an exact square chunk footprint.

## Outputs

- One RGB PNG composite:
  - ocean and lake areas use blue ranges. Ocean-owned coast transition (`CoastOcean`) also stays
    blue; sandy/pale yellow colors are reserved for land-side coast meaning.
  - dry basins use muted olive/khaki fill and are not rendered as water.
  - coast areas use sandy transition colors only on land-side coast surfaces and coast guide overlays.
  - inland land and island components use green-to-upland colors.
  - high elevation approaches white, and peak colors are white.
  - all graph Voronoi edges are drawn as a faint base overlay from actual corner-to-corner graph geometry.
  - ridge candidate edges are white overlays.
  - fault candidate edges are red overlays.
  - coast candidate edges are sandy overlays.
  - every Voronoi edge has one canonical noisy curve from stage 7 boundary realization. This
    boundary overlay is not a straight edge resample: endpoints stay anchored while interior points
    receive deterministic edge-normal displacement large enough to be visible in the default 4K
    preview. The displacement is correlated and smoothed rather than independent per-sample jitter,
    so coast/material/river paths read as natural curves instead of jagged polylines.
  - selected river chains are drawn from hydrology `GraphRiverSegment` results as cyan/blue
    overlays following each segment edge id's canonical noisy curve.
  - selected river sources are drawn as ring markers above the river line. A mainstem source marker
    is chosen once per selected river terminal/system from the upstream-most source trace, while
    tributary source markers are sources whose downstream trace reaches a confluence before a
    terminal. Interior selected river corners are not treated as sources even if their debug node
    kind is `Source`.
  - selected river chains are drawn only for macro edges whose lake edge class is `NonLake`; lake
    internal, lake boundary, and lake-adjacent land edges are never rendered as selected rivers.
  - river width and opacity follow the hydrology segment's selected/display `flow_accumulation`,
    not `raw_flow_accumulation`; lake terminal discharge caps are therefore visible in the preview.
  - lake terminal/inlet selection is already reduced by hydrology before rendering: per-lake
    top-N chain limits, lake-area inlet raw-flow thresholds, and lake-area discharge caps all
    affect the displayed segment set. A qualifying lake-bound trunk is not clipped to only the
    final few lake-adjacent segments; it can remain visible until the land-side inlet endpoint.
  - tiny 1..3-cell local-minima lakes may appear away from rivers. These are macro_map
    `LakeCandidate` components promoted by deterministic low-probability graph minima policy, not
    selected river artifacts.
  - lake inlet, lake outlet, sink, and coast outlet drainage nodes are marked with small
    overlay symbols whose colors are intentionally distinct from the selected river stroke. Internal
    lake debug nodes are hidden in the default preview. Lake inlet/outlet markers are selected graph endpoint markers: inlet markers
    require an incoming selected segment on the land side of the lake boundary, and outlet markers
    require an outgoing selected segment on the land side. Lake boundary edges themselves are never
    drawn as selected river segments. Lake inlets and outlets also draw short directional arrows:
    inlet arrows point into the endpoint from the selected river side, and outlet arrows point away
    from the endpoint toward downstream. These arrows are shortened near the endpoint so they do not
    lay a long shaft over a lake edge. These markers are drawn above the river line so topology
    changes are visible in the preview.
  - selected river source markers are drawn as rings above the river line. A connected selected river
    system gets one mainstem source marker, chosen from its source traces by strongest first segment
    display flow with deterministic tie-breaks. Additional source nodes that merge into that selected
    system through a `Confluence` node are marked as tributary sources. The preview does not treat
    every `GraphDrainageNodeKind::Source` as a mainstem marker.
  - selected river occupancy is stricter than the raw flow ledger: if multiple selected upstream
    branches would share the same corner as separate visible chains, hydrology keeps the largest
    branch and prunes the losing upstream selected tree until explicit confluence geometry exists.
  - selected river lake contact is chain-scoped: a chain ends at a lake inlet, an outlet starts a
    new chain, and any selected path that would touch a second lake contact is pruned.
  - connected selected flow adjacent to a lake must be classified as either a `LakeInlet` or a
    `LakeOutlet`; unclassified lake-connected flow is reported in metadata/stdout and should be 0.
- A compact in-image legend with an elevation color bar and overlay keys.
- A compact compass overlay using the common macro-field/world topdown orientation: image top=N
  (`world +Z`), right=E (`world +X`), bottom=S, left=W.
- A PNG iTXt chunk named `new-world-preview-header` containing seed, generator version, stage,
  center chunk, derived center world block, radius, dimensions, effective world span, graph region sizing, land/ocean tuning values, graph area, site
  count, visible site count, land site count, land ratio, candidate edge count, coast/ridge/fault
  edge counts, selected river/lake/sink/outlet counts, lake/ocean terminal segment counts,
  lake component count, small lake component count, inland water site count, dry basin site
  count, max lake component size, large lake component count, ocean component count, lake-capped
  segment count, source marker mainstem/tributary/drawn counts, lake inlet/outlet count, disconnected lake inlet/outlet count, selected lake-edge
  river segment count, invalid lake contact/intersection count, ambiguous shared corner count,
  duplicate trunk pruned count, repeated lake contact pruned count, unclassified lake-connected flow count,
  selected river source marker mainstem/tributary/drawn counts,
  lake/ocean max display/raw flow, lake inlet raw/display flow range, boundary curve count,
  boundary profile counts, boundary guard violation count, boundary average/max amplitude blocks,
  boundary average/max pixel displacement at the current preview scale, boundary nearly-straight
  curve count, sea level, and source notes.

## Output Path Rules

- With no `--output`, the binary writes `target/macro-map-preview/s<seed>_cx<cx>_cz<cz>_r<r>.png`.
- With `--output <path>.png`, the binary writes that exact PNG path.
- With `--output <directory>`, the binary writes `s<seed>_cx<cx>_cz<cz>_r<r>.png` below that directory.
- Width, height, effective span, spacing, stage, and generator version stay in PNG metadata rather than default filenames.

## Current Flow

1. Parse required seed, center chunk, and inclusive chunk radius.
2. Convert the center chunk to its center world block and resolve the preview window from image dimensions and `(2r + 1) * CHUNK_EDGE` horizontal span.
3. Build a padded Delaunay/circumcenter Voronoi dual graph patch through `generate_voronoi_graph_patch(...)`.
4. Build the macro map through `generate_macro_map(&patch, MacroMapConfig::new(...))`, overriding
   `land_bias` from CLI options when provided.
   - The launch default land bias targets roughly 6:4 land/water in the default macro preview
     window while preserving complex coastlines from graph continentality rather than adding a
     separate continent noise source.
5. Solve selected hydrology through `solve_hydrology(&patch, &macro_map, HydrologyConfig::default())`.
6. Generate canonical noisy boundary curves through `generate_noisy_boundaries(&patch, &macro_map, BoundaryConfig::new(...))`.
7. Generate the RGB pixel buffer with Rayon.
8. Draw a faint base Voronoi edge overlay by resolving each graph edge's corners against the graph
   patch's `VoronoiCorner.position` values, clipping the world-space segment to the preview window,
   and projecting it onto pixel centers.
9. Draw the stage 7 noisy boundary overlay for every edge from `BoundaryCache`. The overlay follows
   the canonical noisy polyline and reports displacement stats in stdout/metadata. The curve points
   are already smoothed/coherent in world space; preview drawing must not replace them with straight
   corner-to-corner segments.
10. Draw candidate edge overlays by resolving `MacroEdge.corners` against the graph patch's
   `VoronoiCorner.position` values, clipping the world-space segment to the preview window, and
   projecting it onto pixel centers. This layer draws graph-derived coast, ridge, and fault guide overlays.
11. Draw selected river chains by following each selected segment's canonical noisy edge curve.
12. Draw selected river source markers and lake/inlet/outlet/sink/coast-outlet drainage node markers
    from hydrology results.
13. Draw the compact legend and encode PNG metadata.

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
new_world::world::generation::generate_noisy_boundaries(&patch, &macro_map, BoundaryConfig::new(seed, generator_version))
```

Goal contract: this preview should show how graph base `continentality/elevation_seed` resolves into
continent/ocean/island ownership, macro elevation, coast, ridge/fault guide, and selected hydrology.
The graph continentality and elevation maps should visibly match the macro map's land/ocean and
high/low patterns, and selected rivers should follow continuous downstream chains to ocean/coast or
explicit lake/sink resolution. Lake-bound selected rivers should terminate at `LakeInlet` vertices,
and lake outlet rivers should start from separated `LakeOutlet` vertices. These vertices are
land-side selected endpoints paired with a lake component; selected river segments must not skim
along or cross lake boundary/internal/adjacent edges. `GraphDrainageNodeKind::Lake` means a graph
local minimum resolved as an internal lake target; it is not a visible river endpoint and is not
drawn by the default preview.
Stage 7 does not create a separate river curve. It creates one canonical noisy curve for every
Voronoi edge, and the hydrology overlay samples the curve for each selected river segment edge id.

If a future worker adds a preview-specific request type, keep this CLI and output path contract stable and replace only the internal map construction.

## Example

```bash
cargo run --bin macro_map_preview -- 42 0 0 512
```

Lower-resolution smoke check:

```bash
cargo run --bin macro_map_preview -- 42 0 0 10 --width 640 --height 360 --output target/macro-map-preview/smoke.png
```
