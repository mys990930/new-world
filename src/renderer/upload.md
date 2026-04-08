# upload

## Role

- Define renderer mesh/upload DTOs and shared vertex layout

## Responsibilities

- `ChunkCoord`
- `RenderBounds`
- `MeshVertex`
- `CpuMesh`
- `GpuChunkMesh`
- `RenderUploadRequest`
- `RenderUploadError`
- Chunk mesh bookkeeping API

## Non-Responsibilities

- Meshing algorithms
- Frame draw encoding
- Gameplay state ownership

## Invariants

- `MeshVertex` uses `#[repr(C)]` plus `Pod`/`Zeroable` so it can be written directly to GPU buffers
- `MeshVertex::vertex_buffer_layout()` is the shared baseline layout for the player-cube pipeline and future chunk pipelines
- `MeshVertex` currently carries `position`, `color`, and `normal`
- The chunk upload path is still metadata/cache bookkeeping only

## Related Modules

- `frame.rs`
- `state.rs`

## Notes

- The first on-screen geometry path still comes from `RenderCubeInstance`-based dynamic cube drawing.
