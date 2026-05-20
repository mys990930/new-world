# registry

## Role

- Interpret file-based block definitions and texture-tile catalogs on the world side
- Centralize gameplay/render meaning lookup for `BlockId`

## Responsibilities

- define `BlockRegistry`, `BlockDef`, and `TextureTileDef`
- define block key/id lookup rules
- define texture key/layer lookup rules
- parse TOML manifests and relative file paths
- reserve the built-in white tile at layer `0`
- define the missing-block fallback
- define block visual-material classification through `BlockMaterialKind`
- define per-block exposed surface-height defaults for meshing
- define `foliage_cross` as a render kind for alpha-cutout vines and thin vegetation

## Non-Responsibilities

- PNG decode
- GPU texture creation or upload
- loaded chunk ownership
- meshing execution scheduling

## Owned Data

### `TextureTileDef`

- `id`
- `key`
- `source`

### `BlockDef`

- `id`
- `key`
- `solid`
- `opaque`
- `render_kind`
- `material`
- `surface_height`
- `face_textures`
- `tint`

### `BlockRegistry`

- `tile_size`
- ordered texture tile list
- block table indexed by `BlockId`
- block/texture key lookup maps
- fallback `__missing` block definition

## Inputs

- manifest path
- TOML text
- manifest-relative texture and block-definition paths

## Outputs

- loaded `BlockRegistry`
- `BlockDef` / `TextureTileDef` lookup results
- `BlockRegistryError`

## Public Interface

```rust
BlockRegistry::load_default() -> Result<BlockRegistry, BlockRegistryError>
BlockRegistry::load_from_path(path: impl AsRef<Path>) -> Result<BlockRegistry, BlockRegistryError>

BlockRegistry::tile_size(&self) -> u32
BlockRegistry::texture_tiles(&self) -> &[TextureTileDef]
BlockRegistry::block_id(&self, key: &str) -> Option<BlockId>
BlockRegistry::texture_id(&self, key: &str) -> Option<TextureTileId>
BlockRegistry::block(&self, id: BlockId) -> Option<&BlockDef>
BlockRegistry::block_or_missing(&self, id: BlockId) -> &BlockDef
BlockRegistry::is_solid(&self, id: BlockId) -> bool
BlockRegistry::is_opaque(&self, id: BlockId) -> bool

default_manifest_path() -> PathBuf
```

## Invariants

- `tile_size` must be non-zero
- `surface_height` must remain within `(0.0, 1.0]`
- texture layer `0` is always the built-in white tile
- texture keys and block keys must be unique
- the `air` block must exist at `id = 0`
- registry lookups expose meaning only and never mutate world storage
- block material classification lives alongside block definition data, not in renderer-only code
- alpha-cutout foliage blocks should normally set `opaque = false` so they do not cull neighboring geometry as if every texel were solid

## Related Modules

- `world.md`
- `chunk.md`
- `generation.md`
- `meshing.md`
- `../../assets/blocks/index.toml`

## Notes

- Block definition TOML files may now specify `material = "..."`.
- Block definition TOML files may now specify `surface_height = 0.90`-style exposed top heights.
- Block definition TOML files may now specify `render = "foliage_cross"` for vines and similar cutout vegetation that keeps one-block world meaning but emits crossed quads in meshing.
- Cube-rendered leaf blocks may also use alpha-cutout PNG textures; they stay `render = "cube"` but should use `opaque = false` so holes can reveal rear faces or neighboring blocks.
- If a block definition omits `material`, the registry infers a reasonable default from the block key so the system remains backward-compatible.
- The current built-in categories are `generic_opaque`, `grass`, `soil`, `stone`, `sand`, `foliage`, `water`, and `emissive`.
- `BlockMaterialKind` exists so meshing and renderer shading can react to material semantics without making the renderer responsible for block-type ownership.
- The registry stores the exposed-top default only; world meshing still decides the final emitted height contextually for stacked fluids.
