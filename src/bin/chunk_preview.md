# chunk_preview

## Role

- Render a fixed-angle offscreen preview of generated chunks.
- Support both direct seed generation and rendering from a previously created world dump.

## Inputs

- either `seed` or `--world-dir <path>`
- optional `--stage <full|prototype|hydrology>` when previewing from a seed
- preview center in chunk coordinates
- horizontal render radius
- vertical chunk bounds
- optional coarse preview density via `--lod-blocks <u8>`
- output image path

## Outputs

- a PNG image rendered through the offscreen renderer
- a compass overlay using the shared preview orientation: image top=N (`world +Z`), right=E (`world +X`), bottom=S, left=W
- a stdout summary for the center chunk, including the sampled surface `y` average/min/max, resolved region classification, atlas sample values, and meso guide weights

## Current Flow

1. Resolve the preview source.
2. Load or generate the requested chunk window plus padding for meshing.
   - Direct-seed generation builds a per-preview generation input cache keyed by generation atlas area, so neighboring chunks and vertical chunk columns reuse identical atlas / region / meso inputs instead of rebuilding them.
3. Build either full block meshes through `world::meshing`, a post-prototype meso-applied heightfield mesh for `--stage prototype`, a post-hydrology carved terrain mesh with water overlays for `--stage hydrology`, or a coarse probe-driven heightfield mesh when `--lod-blocks > 1`.
4. Render them with a fixed quarter-view camera and a relief-friendly preview lighting setup.
5. Save the PNG to disk.
6. Print center-chunk diagnostics so preview images can be correlated with the generator's biome / atlas state.

- chunk loading / generation and preview mesh preparation may fan out across multiple CPU cores, but the output image remains deterministic for the same inputs.
- the seed-backed `full`, `prototype`, and `hydrology` paths reuse cached generation inputs only when the underlying generation atlas area is identical; this preserves the same terrain semantics as per-chunk input assembly.

## Prototype Stage

- `--stage prototype` is seed-only and renders the post-prototype meso-applied heightfield instead of realized chunk meshes.
- The prototype path builds `ChunkGenerationInputs` for each chunk in the requested window, then calls the generation scaffold, base-heightfield prototype solve, and meso apply stage before meshing the result for preview.
- Vertical chunk bounds are ignored in prototype mode; the render footprint is controlled by `--center-x`, `--center-z`, and `--radius`.
- The default output name still includes `_prototype` so the image is easy to tell apart from the full chunk preview, even though the rendered surface already includes meso deformation.
- The stdout diagnostics still report the center chunk's atlas / region / meso sample, and the surface summary is derived from the prototype heightfield columns rather than realized blocks.

Example:

```bash
cargo run --bin chunk_preview -- 42 --stage prototype --center-x 4 --center-z -3 --radius 2 --output target/chunk-preview/prototype.png
```

## Hydrology Stage

- `--stage hydrology` is seed-only and renders the post-smoothing hydrology carve plus visible water overlays.
- The hydrology path builds the same generation scaffold, then runs prototype, meso apply, smoothing, and `build_chunk_hydrology_solve(...)` before meshing the preview surface.
- Terrain columns render at the carved hydrology-adjusted height, and water columns render a separate top sheet from the solved `water_surface_height` using the water block texture/material.
- The stdout diagnostics include an extra hydrology line with water / channel / floodplain / lake / wetland counts for the center chunk.

Example:

```bash
cargo run --bin chunk_preview -- 42 --stage hydrology --center-x 40 --center-z -29 --radius 2 --output target/chunk-preview/hydrology.png
```

## Default Vertical Window

- The current default preview window is chunk `y = -2..3`, which keeps rendering focused on approximately world `y > -40` while still capturing surface relief.

## LOD Preview Notes

- `--lod-blocks 4` means one rendered coarse column covers `4 x 4` world blocks in plan view.
- The current LOD path samples the generator's probe surface directly and renders a wide-angle scaffold mesh without materializing every full block face.
- The current LOD path applies a preview-only vertical exaggeration so broad isometric shots still show sea shelves, uplands, and ridge relief.
- LOD preview is currently supported for direct seed previews; created-world previews still use the full mesh path.
- LOD, prototype, and hydrology stages are mutually exclusive; prototype and hydrology previews currently use the full block-resolution surface rather than probe LOD sampling.
- For an exact `xz` top-down PNG that scans realized chunk data rather than probe surfaces, use `chunk_topdown_preview`.
