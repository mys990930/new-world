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
- `MeshVertex` currently carries `position`, `color`, `normal`, `uv`, and `texture_layer`
- `RenderUploadRequest::UpsertChunkMesh` validates the CPU mesh and stores enough data to rebuild GPU buffers after live surface attach
- If there is no live backend yet, the renderer may cache the CPU mesh first and build GPU buffers later

## Related Modules

- `frame.rs`
- `state.rs`
- `surface.rs`
- `texture.rs`

## Notes

- The current vertical slice uses `RenderUploadRequest` to move generated chunk plane meshes into the renderer cache before `frame.rs` draws them.
- `texture_layer` indexes the renderer-owned block texture array, and dynamic cubes currently use layer `0` with tint color.
