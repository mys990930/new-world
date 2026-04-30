# graph_voronoi_preview

## Role

- Render deterministic 4K top-down PNG previews for the graph-first Voronoi macro graph stage and current site-field maps.
- Preserve detailed preview configuration in PNG metadata while keeping default filenames short.
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
  - `--mode <all|identity|temperature|hydration|humidity|continentality|elevation|ruggedness>`
  - `--output <path>`

## Defaults

- `--width 3840`
- `--height 2160`
- `--world-span-blocks 32768`
- `--region-size-blocks DEFAULT_GRAPH_REGION_SIZE_BLOCKS`
- `--site-spacing-blocks DEFAULT_SITE_SPACING_BLOCKS`
- `--stage graph_voronoi`
- `--mode identity`
- single mode output: `target/graph-voronoi-preview/s<seed>_x<center-x>_z<center-z>_<mode>.png`
- all-mode output directory: `target/graph-voronoi-preview/s<seed>_x<center-x>_z<center-z>/`

## Outputs

- `--mode identity` emits one PNG image where:
  - site ownership is shown as a stable color field
  - nearest-site raster boundaries are darkened as a diagnostic fill cue
  - actual graph topology is overlaid from `VoronoiEdge.corners` corner-to-corner geometry
  - site centers are highlighted
  - graph region cache boundaries are lightly marked
- single field modes emit one PNG with a numeric color gradient sampled from smoothed `VoronoiSite::base_fields` where applicable:
  - `temperature`: cold to warm, blue through pale neutral to red
  - `hydration` / `humidity`: dry to wet, ochre through green to blue
  - `continentality`: oceanic to continental, blue coastal colors through inland greens/browns
  - `elevation`: low to high, lowland water/green through upland and snow colors
  - `ruggedness`: flat to rough, green/yellow through rock gray
- `--mode all` emits `identity`, `temperature`, `hydration`, `continentality`, `elevation`, and `ruggedness` PNG files in an output directory.
- each PNG includes a compact legend overlay in one corner:
  - identity mode shows a small map label/header only
  - field modes show a small gradient bar with low/high meaning labels
- stdout summary for seed, generator version, selected modes, world footprint, graph region area, site count, metadata, and generated file paths
- a PNG iTXt chunk named `new-world-preview-header` containing the deterministic header fields plus `mode` and `map_name`

## Output Path Rules

- In a single mode with no `--output`, the binary writes `target/graph-voronoi-preview/s<seed>_x<center-x>_z<center-z>_<mode>.png`.
- In `all` mode with no `--output`, the binary creates `target/graph-voronoi-preview/s<seed>_x<center-x>_z<center-z>/`.
- In `all` mode, `--output` must be a directory path.
- In a single mode, `--output <path>.png` writes that file.
- In a single mode, `--output <directory>` writes `<mode>.png` below that directory.
- Width, height, span, spacing, stage, and generator version stay in PNG metadata rather than the default filename.

## Current Flow

1. Parse the required seed and world-block center.
2. Resolve the graph preview window from image dimensions and `--world-span-blocks`.
3. Build a `VoronoiGraphPatch` through `generate_voronoi_graph_patch(...)`.
   - The preview derives the required padding from the requested image footprint so the visible area has surrounding sites.
4. Generate the RGB pixel buffer with Rayon via parallel chunks.
5. Convert that buffer through `image::RgbImage`.
6. Draw the actual graph edge/corner overlay from explicit `VoronoiEdge` and `VoronoiCorner` topology.
7. Draw the compact legend overlay directly into the RGB image without external font dependencies.
8. Encode PNG with the `png` crate so the header is preserved as metadata.

The temperature, hydration, continentality, and elevation modes read the graph base-field stage's smoothed `VoronoiSite::base_fields`. Ruggedness remains a site-level graph roughness seed until a later terrain stage derives a richer roughness field.

Identity mode still uses nearest-site raster color fill because it is useful for inspecting site ownership.
The overlaid edges are the source-of-truth Delaunay/circumcenter Voronoi dual topology, so the darkened
raster tie lines and the explicit edge overlay are allowed to differ.

## Example

```bash
cargo run --bin graph_voronoi_preview -- 42 0 0
```

Lower-resolution smoke check with the default macro-map-aligned world span:

```bash
cargo run --bin graph_voronoi_preview -- 42 0 0 --width 960 --height 540
```

All-map directory output:

```bash
cargo run --bin graph_voronoi_preview -- 42 0 0 --mode all --width 960 --height 540 --output target/graph-voronoi-preview/s42_maps
```
