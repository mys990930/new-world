# meshing

## Role

- Convert world-owned chunk snapshots into renderer-ready CPU mesh payloads
- Define the last world-side boundary before app/jobs translate into renderer uploads

## Responsibilities

- define the center + neighbor snapshot input shape
- interpret render kind, opacity, face textures, tint, and material kind through `BlockRegistry`
- resolve context-sensitive exposed surface height for partial-height materials such as water
- emit crossed alpha-cutout quads for `foliage_cross` render-kind blocks such as vines
- decide which faces are visible
- emit a world-owned `CpuMesh`
- handle chunk-border visibility through neighbor snapshots
- encode top-face terrace contour edges for renderer-side readability shading

## Non-Responsibilities

- visible chunk calculation
- GPU upload
- draw-call encoding
- world mutation

## Inputs

- center `ChunkSnapshot`
- neighbor chunk snapshots
- `BlockRegistry`

## Outputs

- world-owned `CpuMesh`
  - `MeshVertex { position, color, normal, uv, texture_layer, material_kind, contour_edges }`

## Processing Flow

1. If the center chunk is uniform empty/non-rendered, return an empty `CpuMesh`.
2. If the center chunk is uniform full-height opaque, inspect only chunk boundary cells because all interior faces are hidden by the center block itself.
3. Otherwise iterate every block in the center chunk snapshot.
4. Resolve render kind, tint, face texture, opacity, block material, and exposed surface height through the registry.
5. For `foliage_cross`, emit two double-sided crossed quads with alpha-aware foliage material and skip cube face culling.
6. Cull cube faces hidden by opaque neighbors or fully shared fluid volume.
7. Emit face vertices with position, tint, normal, UV, texture layer, material kind, and any top-face contour-edge mask needed for renderer shading.
8. Return the accumulated `CpuMesh`.

## Public Interface

```rust
meshing::build_chunk_mesh(
    center: &ChunkSnapshot,
    neighbors: NeighborChunks,
    registry: &BlockRegistry,
) -> CpuMesh
```

## Invariants

- meshing never mutates input snapshots
- border culling must respect neighbor chunk presence and opacity
- the result is CPU-side data only and does not own GPU resources
- material classification comes from `BlockRegistry`; meshing only copies it into the vertex payload
- partial-height geometry remains world-owned meaning; the renderer must not invent lowered water surfaces on its own
- `foliage_cross` blocks remain world blocks, but their render geometry is proxy foliage geometry rather than cube-derived faces

## Related Modules

- `chunk.md`
- `query.md`
- `jobs`
- `renderer`

## Notes

- The current implementation still emits cube-derived quads, but may lower exposed top surfaces and clip shared side faces for water blocks.
- `foliage_cross` vines emit two crossed vertical quads, keep foliage material classification, and rely on renderer alpha cutout rather than block-by-block cube silhouettes.
- Tree leaf blocks remain cube-rendered foliage blocks so crowns preserve a readable voxel silhouette in quarter view; their block definitions are non-opaque so alpha-cutout texture holes do not behave like fully solid occluders.
- Uniform air chunks return immediately, and uniform full-height opaque chunks mesh only their six boundary faces instead of scanning all `32^3` blocks.
- Missing block ids resolve through the registry fallback and therefore produce a magenta-tinted mesh with a valid material kind.
- World meshing intentionally keeps `material_kind` as world-owned meaning so the app bridge can translate it into renderer-specific shading enums without leaking world internals.
- Terrace contour readability is now driven by a world-produced top-edge bitmask, so shaders can highlight real height breaks on top faces without reverting to per-block outlines.
