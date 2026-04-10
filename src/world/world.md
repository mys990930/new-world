## world

### Role

- Own the source-of-truth block and chunk data for the game world
- Interpret block definitions, texture-tile lookup, and block material classification through `BlockRegistry`
- Expose read/write APIs and read-only snapshot/query surfaces without leaking raw storage ownership

### Responsibilities

- loaded chunk storage
- world metadata storage
- block registry storage
- block/chunk read-write API
- coordinate transformation rule ownership
- atlas-scale macro environment interpretation
- snapshot/query surface provision
- edit result / dirty chunk calculation
- procedural generation result expression as `ChunkData`
- block definition / texture-tile lookup contract
- block visual-material lookup contract
- save/load serialization contract
- meshing input provision from chunk snapshot bundle
- block-grid raycast query provision

### Non-Responsibilities

- visible chunk calculation
- gameplay command interpretation
- fixed tick scheduling
- async worker orchestration
- gpu buffer init / draw call

### Owned Data

- `WorldMeta`
- `ChunkCoord`, `LocalBlockCoord`, `WorldBlockCoord`
- `BlockId`, `BlockFace`
- `BlockDef`, `BlockRegistry`
- `BlockMaterialKind`
- `TextureTileId`, `TextureTileDef`, `TextureTileSource`
- `ChunkData`, `ChunkSnapshot`
- `AtlasCoord`, `AtlasArea`
- `AtlasFieldMap`, `AtlasResolvedMap`
- `WorldEdit`, `EditResult`
- `MeshVertex`, `CpuMesh`, `RenderBounds`
- `NeighborChunks`
- `Ray3`, `RaycastHit`
- `WorldCore`

### Public Interface

```rust
BlockRegistry::load_default() -> Result<BlockRegistry, BlockRegistryError>
BlockRegistry::load_from_path(path: impl AsRef<Path>) -> Result<BlockRegistry, BlockRegistryError>

WorldCore::new(meta: WorldMeta, block_registry: Arc<BlockRegistry>) -> WorldCore
WorldCore::block_registry(&self) -> &BlockRegistry
WorldCore::block_registry_handle(&self) -> Arc<BlockRegistry>

WorldCore::has_chunk(coord: ChunkCoord) -> bool
WorldCore::insert_chunk(coord: ChunkCoord, chunk: ChunkData)
WorldCore::remove_chunk(coord: ChunkCoord) -> Option<ChunkData>

WorldCore::get_block(pos: WorldBlockCoord) -> Option<BlockId>
WorldCore::apply_edit(edit: WorldEdit) -> EditResult

WorldCore::get_chunk(coord: ChunkCoord) -> Option<&ChunkData>
WorldCore::get_chunk_mut(coord: ChunkCoord) -> Option<&mut ChunkData>
WorldCore::snapshot_chunk(coord: ChunkCoord) -> Option<ChunkSnapshot>

WorldCore::snapshot_region(...)
WorldCore::query_neighbors(...)
WorldCore::query_block_state(...)
WorldCore::raycast_blocks(ray: Ray3, max_distance: f32) -> Option<RaycastHit>

generation::generate_chunk(
    coord: ChunkCoord,
    meta: &WorldMeta,
    registry: &BlockRegistry,
) -> ChunkData

atlas::generate_atlas_fields(
    meta: &WorldMeta,
    area: AtlasArea,
) -> AtlasFieldMap

atlas::resolve_atlas(fields: &AtlasFieldMap) -> AtlasResolvedMap

storage::load_chunk(bytes: &[u8]) -> Result<ChunkData, StorageError>
storage::save_chunk(snapshot: &ChunkSnapshot) -> Result<Vec<u8>, StorageError>

meshing::build_chunk_mesh(
    center: &ChunkSnapshot,
    neighbors: NeighborChunks,
    registry: &BlockRegistry,
) -> CpuMesh
```

### Dependencies

- save format config

NOT:

- `app`
- `ecs`
- `renderer`
- `platform`

### Invariants

1. Block and chunk mutations only happen through world-owned APIs.
2. Raw chunk storage keeps ids, while render and gameplay meaning is interpreted through `BlockRegistry`.
3. `BlockRegistry` owns face-texture lookup and visual material lookup for each block definition.
4. World meshing produces CPU-side mesh data only; it does not own GPU resources.
5. World-owned mesh vertices may carry render-facing metadata such as `uv`, `texture_layer`, and `material_kind`, but the renderer still owns GPU formats and shading policy.

### Submodules

- `meta.md`: `WorldMeta` seed and version contract
- `coord.md`: world/chunk/local coordinate conversion rules
- `chunk.md`: `ChunkData` / `ChunkSnapshot` structure and invariants
- `core.md`: `WorldCore` ownership and top-level API
- `edit.md`: `WorldEdit` / `EditResult` mutation contract
- `query.md`: read-only block/chunk/region/raycast surface
- `registry.md`: data-driven block definition, texture tile, and block material contract
- `generation.md`: chunk generation rules
- `atlas/atlas.md`: atlas prototype contracts
- `storage.md`: serialization contract
- `meshing.md`: snapshot-to-CPU-mesh contract

### Current Implementation Notes

- The default block registry is loaded from `assets/blocks/index.toml`.
- Registry entries now carry an explicit or inferred `BlockMaterialKind` in addition to face textures and tint.
- Chunk generation now consumes atlas fields inside `world::generation` and realizes a stone-only first-pass relief model around fixed sea level `y = 0`.
- Meshing emits `material_kind` per vertex so renderer shaders can react differently to grass, soil, stone, and future categories without the renderer owning block semantics.
- Renderer conversion still happens through `jobs/app::bridge`; world does not upload directly to the GPU.
