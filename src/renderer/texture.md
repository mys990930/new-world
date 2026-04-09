# texture

## Role

- Define renderer-side block texture source DTOs and texture-array upload policy

## Responsibilities

- `RenderTextureSource`
- `RenderTextureTile`
- `RenderTextureArraySource`
- `RenderTextureError`
- CPU-side texture decoding and validation
- `texture_2d_array` GPU resource creation

## Non-Responsibilities

- Defining block meaning
- Parsing world block manifests
- Meshing algorithms
- Frame visibility decisions

## Public Interface

```rust
Renderer::set_block_textures(
    source: RenderTextureArraySource,
) -> Result<(), RenderTextureError>
```

## Invariants

- All layers in a texture array share the same `tile_size`
- Layers must start at `0` and be contiguous
- The current renderer uploads block textures as `Rgba8UnormSrgb`
- Sampling uses `nearest` filtering for both min/mag paths

## Related Modules

- `renderer.md`
- `state.rs`
- `surface.rs`
- `upload.rs`

## Notes

- Layer `0` is typically the built-in white tile used by debug cubes and tint-only geometry.
- File-backed textures are currently decoded from PNG and validated against the declared `tile_size`.
- `RenderTextureArraySource` is renderer-owned DTO data; world still talks in terms of `TextureTileId` and manifest-backed sources.
