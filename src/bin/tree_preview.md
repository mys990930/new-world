# tree_preview

## Role

- render several `world::tree` blueprint variants through the existing offscreen renderer
- provide a fast visual diagnostic for per-climate tree rules and block palettes

## Inputs

- positional: `<tree-kind> <preview-seed>`
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
- a compass overlay using the shared preview orientation: image top=N, right=E, bottom=S, left=W
- stdout summary of kind, preview seed, generated per-tree seeds, voxel counts, bounds, and output path

## Defaults

- `--width 1600`
- `--height 1000`
- `--quarter-turns 0`

## Example

```bash
cargo run --bin tree_preview -- jungle 42 --output target/tree-preview/jungle.png
```

## Notes

- This tool does not mutate or load a `WorldCore`.
- It generates five same-kind tree variants from deterministic seeds derived from the preview seed.
- The five variants are spaced 36 blocks apart, matching the current `1 block = 0.5m` scale and leaving room for wider crowns.
- It converts the generated relative tree voxels into temporary chunks only for meshing and rendering.
- The preview ground is a temporary `grass` block plane.
- The preview camera uses a lower-than-gameplay angle and tighter framing so tree silhouettes and block faces read clearly.
- The preview environment intentionally uses a muted diagnostic sunset-style light and darker clear color, closer to the CPU isometric heightfield preview mood than the bright gameplay midday preset.
- The block palette comes from the default block registry and uses the climate-specific test tree blocks under `assets/blocks`.
- Tree leaves remain cube-rendered foliage blocks with muted alpha-cutout leaf textures, while vine blocks use `render = "foliage_cross"` cutout quads.
