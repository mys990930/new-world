# chunk_preview

## Role

- Render a fixed-angle offscreen preview of generated chunks.
- Support both direct seed generation and rendering from a previously created world dump.

## Inputs

- either `seed` or `--world-dir <path>`
- optional `--stage <full|prototype>` when previewing from a seed
- preview center in chunk coordinates
- horizontal render radius
- vertical chunk bounds
- optional coarse preview density via `--lod-blocks <u8>`
- output image path

## Outputs

- a PNG image rendered through the offscreen renderer

## Current Flow

1. Resolve the preview source.
2. Load or generate the requested chunk window plus padding for meshing.
3. Build either full block meshes through `world::meshing`, a post-prototype meso-applied heightfield mesh for `--stage prototype`, or a coarse probe-driven heightfield mesh when `--lod-blocks > 1`.
4. Render them with a fixed quarter-view camera and a relief-friendly preview lighting setup.
5. Save the PNG to disk.

- chunk loading / generation and preview mesh preparation may fan out across multiple CPU cores, but the output image remains deterministic for the same inputs.

## Prototype Stage

- `--stage prototype` is seed-only and renders the post-prototype meso-applied heightfield instead of realized chunk meshes.
- The prototype path builds `ChunkGenerationV2Inputs` for each chunk in the requested window, then calls the V2 scaffold, base-heightfield prototype solve, and meso apply stage before meshing the result for preview.
- Vertical chunk bounds are ignored in prototype mode; the render footprint is controlled by `--center-x`, `--center-z`, and `--radius`.
- The default output name still includes `_prototype` so the image is easy to tell apart from the full chunk preview, even though the rendered surface already includes meso deformation.

Example:

```bash
cargo run --bin chunk_preview -- 42 --stage prototype --center-x 4 --center-z -3 --radius 2 --output target/chunk-preview/prototype.png
```

## Default Vertical Window

- The current default preview window is chunk `y = -2..3`, which keeps rendering focused on approximately world `y > -40` while still capturing surface relief.

## LOD Preview Notes

- `--lod-blocks 4` means one rendered coarse column covers `4 x 4` world blocks in plan view.
- The current LOD path samples the generator's probe surface directly and renders a wide-angle scaffold mesh without materializing every full block face.
- The current LOD path applies a preview-only vertical exaggeration so broad isometric shots still show sea shelves, uplands, and ridge relief.
- LOD preview is currently supported for direct seed previews; created-world previews still use the full mesh path.
- LOD and prototype stages are mutually exclusive; prototype preview currently uses the full block-resolution surface rather than probe LOD sampling.
- For an exact `xz` top-down PNG that scans realized chunk data rather than probe surfaces, use `chunk_topdown_preview`.
