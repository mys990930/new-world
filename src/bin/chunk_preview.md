# chunk_preview

## Role

- Render a fixed-angle offscreen preview of generated chunks.
- Support both direct seed generation and rendering from a previously created world dump.

## Inputs

- either `seed` or `--world-dir <path>`
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
3. Build either full block meshes through `world::meshing` or a coarse probe-driven heightfield mesh when `--lod-blocks > 1`.
4. Render them with a fixed quarter-view camera and a relief-friendly preview lighting setup.
5. Save the PNG to disk.

## Default Vertical Window

- The current default preview window is chunk `y = -2..3`, which keeps rendering focused on approximately world `y > -40` while still capturing surface relief.

## LOD Preview Notes

- `--lod-blocks 4` means one rendered coarse column covers `4 x 4` world blocks in plan view.
- The current LOD path samples the generator's probe surface directly and renders a wide-angle scaffold mesh without materializing every full block face.
- The current LOD path applies a preview-only vertical exaggeration so broad isometric shots still show sea shelves, uplands, and ridge relief.
- LOD preview is currently supported for direct seed previews; created-world previews still use the full mesh path.
- For an exact `xz` top-down PNG that scans realized chunk data rather than probe surfaces, use `chunk_topdown_preview`.
