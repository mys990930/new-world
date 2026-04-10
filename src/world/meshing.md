# meshing

## Role

- Convert world-owned chunk snapshots into renderer-ready CPU mesh payloads
- Define the last world-side boundary before app/jobs translate into renderer uploads

## Responsibilities

- define the center + neighbor snapshot input shape
- interpret render kind, opacity, face textures, tint, and material kind through `BlockRegistry`
- decide which faces are visible
- emit a world-owned `CpuMesh`
- handle chunk-border visibility through neighbor snapshots

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
  - `MeshVertex { position, color, normal, uv, texture_layer, material_kind }`

## Processing Flow

1. Iterate every block in the center chunk snapshot.
2. Resolve render kind, tint, face texture, opacity, and block material through the registry.
3. Cull faces hidden by opaque neighbors.
4. Emit face vertices with position, tint, normal, UV, texture layer, and material kind.
5. Return the accumulated `CpuMesh`.

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

## Related Modules

- `chunk.md`
- `query.md`
- `jobs`
- `renderer`

## Notes

- The current implementation still only emits cube faces.
- Missing block ids resolve through the registry fallback and therefore produce a magenta-tinted mesh with a valid material kind.
- World meshing intentionally keeps `material_kind` as world-owned meaning so the app bridge can translate it into renderer-specific shading enums without leaking world internals.
