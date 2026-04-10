# chunk_preview

## Role

- Render a fixed-angle offscreen preview of generated chunks.
- Support both direct seed generation and rendering from a previously baked world dump.

## Inputs

- either `seed` or `--world-dir <path>`
- preview center in chunk coordinates
- horizontal render radius
- vertical chunk bounds
- output image path

## Outputs

- a PNG image rendered through the offscreen renderer

## Current Flow

1. Resolve the preview source.
2. Load or generate the requested chunk window plus padding for meshing.
3. Build chunk meshes through `world::meshing`.
4. Render them with a fixed quarter-view camera.
5. Save the PNG to disk.

## Default Vertical Window

- The current default preview window is chunk `y = -2..3`, which keeps rendering focused on approximately world `y > -40` while still capturing surface relief.
