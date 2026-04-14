## world

### Role

- own the source-of-truth block and chunk data for the game world
- interpret block definitions, texture tiles, and material classification through `BlockRegistry`
- expose read/write APIs and read-only snapshot/query surfaces without leaking raw storage ownership

### Responsibilities

- loaded chunk storage
- world metadata storage
- block registry storage
- block/chunk read-write API
- coordinate transformation rules
- atlas-scale macro environment interpretation
- atlas-scale mountain-chain / drainage structure ownership
- snapshot/query surfaces
- edit result / dirty chunk calculation
- procedural generation result expression as `ChunkData`
- block definition / texture tile lookup
- block visual-material lookup
- block exposed-surface-height lookup for meshing
- save/load byte codec
- baked world manifest / baked chunk load support
- meshing input provision
- block-grid raycast queries

### Non-Responsibilities

- visible chunk calculation
- gameplay command interpretation
- fixed tick scheduling
- async worker orchestration
- GPU buffer init / draw calls

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
- `AtlasStructureMap`
- `MountainChainGraph`, `MountainSpineSegment`
- `DrainageGraph`, `RiverPathSegment`
- `ChunkGenerationProbe`, `ColumnGenerationProbe`
- `WorldEdit`, `EditResult`
- `MeshVertex`, `CpuMesh`, `RenderBounds`
- `NeighborChunks`
- `Ray3`, `RaycastHit`
- `BakedWorldManifest`, `BakedStackSummary`, `BakedWorldSource`
- `WorldCore`

### Public Interface
```rust
BlockRegistry::load_default() -> Result<BlockRegistry, BlockRegistryError>
BlockRegistry::load_from_path(path: impl AsRef<Path>) -> Result<BlockRegistry, BlockRegistryError>

generate_atlas_structure(meta: &WorldMeta, area: AtlasArea) -> AtlasStructureMap
generate_atlas_structure_with_tuning(
    meta: &WorldMeta,
    area: AtlasArea,
    tuning: &AtlasTuning,
) -> AtlasStructureMap

WorldCore::new(meta: WorldMeta, block_registry: Arc<BlockRegistry>) -> WorldCore
WorldCore::block_registry(&self) -> &BlockRegistry
WorldCore::block_registry_handle(&self) -> Arc<BlockRegistry>
WorldCore::loaded_chunk_bounds(&self) -> Option<(ChunkCoord, ChunkCoord)>

read_baked_world_manifest(root: &Path) -> Result<BakedWorldManifest, BakedWorldError>
load_baked_chunk(root: &Path, coord: ChunkCoord) -> Result<ChunkData, BakedWorldError>
detect_latest_baked_world_root(base_dir: &Path) -> io::Result<Option<PathBuf>>

storage::load_chunk(bytes: &[u8]) -> Result<ChunkData, StorageError>
storage::save_chunk(snapshot: &ChunkSnapshot) -> Result<Vec<u8>, StorageError>
```

### Dependencies

- save format config

NOT:

- `app`
- `ecs`
- `renderer`
- `platform`

### Invariants

1. block and chunk mutations only happen through world-owned APIs
2. raw chunk storage keeps ids while gameplay/render meaning is interpreted through `BlockRegistry`
3. world meshing produces CPU-side data only
4. baked-world helpers may load chunk bytes, but in-memory chunk ownership still belongs to `WorldCore`
5. partial-height block geometry such as lowered exposed water surfaces is decided on the world side before renderer upload
6. chunk-order-independent macro terrain direction such as mountain spines and river paths belongs to atlas/world rather than per-chunk realization code

### Submodules

- `meta.md`: `WorldMeta` seed and version contract
- `coord.md`: world/chunk/local coordinate conversion rules
- `chunk.md`: `ChunkData` / `ChunkSnapshot` structure and invariants
- `core.md`: `WorldCore` ownership and top-level API
- `edit.md`: `WorldEdit` / `EditResult` mutation contract
- `query.md`: read-only block/chunk/region/raycast surface
- `registry.md`: block definition, texture tile, and material contract
- `generation.md`: chunk generation rules
- `atlas/atlas.md`: atlas prototype contracts
- `atlas/structure.md`: atlas-owned mountain-chain and drainage skeleton contract
- `storage.md`: raw chunk byte serialization contract
- `baked.md`: baked manifest / baked runtime load contract
- `meshing.md`: snapshot-to-CPU-mesh contract

### Current Implementation Notes

- the default block registry is loaded from `assets/blocks/index.toml`
- chunk acquisition can now come from either baked disk load or procedural generation before converging back into the same in-memory `WorldCore`
- meshing still operates on snapshots and renderer upload still happens outside `world`
- exposed-water height and top-face terrace contour hints are now produced in world meshing so renderer readability effects stay anchored to world-owned geometry meaning
- atlas terrain structure is currently still scalar-first in code, but the next generation revision is expected to move mountain-chain and drainage direction ownership into atlas before chunk realization
