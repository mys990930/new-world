# upload

## Role

- Define renderer mesh/upload DTOs and the shared vertex layout

## Responsibilities

- `ChunkCoord`
- `RenderBounds`
- `RenderMaterialKind`
- `MeshVertex`
- `CpuMesh`
- `GpuChunkMesh`
- `RenderUploadRequest`
- `RenderUploadError`
- chunk mesh bookkeeping API

## Non-Responsibilities

- meshing algorithms
- frame draw encoding
- gameplay state ownership

## Invariants

- `MeshVertex` uses `#[repr(C)]` plus `Pod`/`Zeroable` so it can be written directly to GPU buffers
- `MeshVertex::vertex_buffer_layout()` is the shared baseline layout for terrain and dynamic pipelines
- `MeshVertex` now carries `position`, `color`, `normal`, `uv`, `texture_layer`, `material_kind`, and `contour_edges`
- `RenderMaterialKind` is renderer-owned shading meaning, separate from world-owned `BlockMaterialKind`
- `RenderUploadRequest::UpsertChunkMesh` validates the CPU mesh and stores enough data to rebuild GPU buffers after live surface attach
- if there is no live backend yet, the renderer may cache the CPU mesh first and build GPU buffers later
- uploaded chunk meshes may be partitioned into opaque and translucent subsets inside the renderer so water can render after opaque terrain without changing app-facing DTOs

## Related Modules

- `frame.rs`
- `state.rs`
- `surface.rs`
- `texture.rs`

## Notes

- `texture_layer` indexes the renderer-owned block texture array
- `material_kind` lets shaders branch on grass / soil / stone / actor / shadow / highlight behavior without querying gameplay state
- `contour_edges` lets the terrain shader apply readability lines only on world-provided top-face height breaks
- dynamic cubes still use texture layer `0` today, but their material kind now carries more of the visual meaning
