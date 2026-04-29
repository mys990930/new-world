# tree_preview

## Role

- render a single `world::tree` blueprint through the existing offscreen renderer
- provide a fast visual diagnostic for per-climate tree rules and block palettes

## Inputs

- positional: `<tree-kind> <seed>`
- optional: `--output <path>`, `--width <u32>`, `--height <u32>`, `--quarter-turns <u8>`

## Tree Kinds

- `polar_tundra_shrub`
- `boreal_taiga_conifer`
- `temperate_deciduous`
- `temperate_birch`
- `mediterranean_olive`
- `swamp_cypress`
- `savanna_acacia`
- `tropical_rainforest_jungle`

Aliases such as `tundra`, `taiga`, `oak`, `birch`, `olive`, `swamp`, `acacia`, and `jungle` are accepted by `TreeKind::from_key`.

## Outputs

- a PNG image rendered through the renderer offscreen path
- stdout summary of kind, seed, voxel count, bounds, and output path

## Example

```bash
cargo run --bin tree_preview -- jungle 42 --output target/tree-preview/jungle.png
```

## Notes

- This tool does not mutate or load a `WorldCore`.
- It converts the generated relative tree voxels into temporary chunks only for meshing and rendering.
- The block palette comes from the default block registry and uses the climate-specific test tree blocks under `assets/blocks`.
