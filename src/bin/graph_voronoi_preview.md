# graph_voronoi_preview

## Role

- Render a deterministic 4K top-down PNG preview for the graph-first Voronoi macro graph stage.
- Preserve the preview header both in PNG metadata and in the default deterministic filename.
- Use the world-owned `world::generation::graph` patch construction API.

## Inputs

- positional: `<seed> <center-x> <center-z>`
  - `center-x` and `center-z` are world-block coordinates.
- optional:
  - `--width <u32>`
  - `--height <u32>`
  - `--world-span-blocks <i32>`
  - `--region-size-blocks <i32>`
  - `--site-spacing-blocks <i32>`
  - `--stage graph_voronoi`
  - `--output <path>`

## Defaults

- `--width 3840`
- `--height 2160`
- `--world-span-blocks 8192`
- `--region-size-blocks DEFAULT_GRAPH_REGION_SIZE_BLOCKS`
- `--site-spacing-blocks DEFAULT_SITE_SPACING_BLOCKS`
- `--stage graph_voronoi`
- `--output target/graph-voronoi-preview/seed_<seed>_cx<center-x>_cz<center-z>_generator_gv<generator-version>_stage_graph_voronoi_<width>x<height>_span<world-span-blocks>_spacing<site-spacing-blocks>.png`

## Outputs

- a PNG image where:
  - site ownership is shown as a stable color field
  - approximated Voronoi boundaries are darkened
  - site centers are highlighted
  - graph region cache boundaries are lightly marked
- stdout summary for seed, generator version, world footprint, graph region area, site count, image dimensions, metadata, and output path
- a PNG iTXt chunk named `new-world-preview-header` containing the same deterministic header fields used by the filename contract

## Current Flow

1. Parse the required seed and world-block center.
2. Resolve the graph preview window from image dimensions and `--world-span-blocks`.
3. Build a `VoronoiGraphPatch` through `generate_voronoi_graph_patch(...)`.
   - The preview derives the required padding from the requested image footprint so the visible area has surrounding sites.
4. Generate the RGB pixel buffer with Rayon via parallel chunks.
5. Convert that buffer through `image::RgbImage`.
6. Encode PNG with the `png` crate so the header is preserved as metadata.

## Example

```bash
cargo run --bin graph_voronoi_preview -- 42 0 0
```

Lower-resolution smoke check:

```bash
cargo run --bin graph_voronoi_preview -- 42 0 0 --width 960 --height 540 --world-span-blocks 4096
```
