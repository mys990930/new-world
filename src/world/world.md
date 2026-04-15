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
- future deterministic meso terrain-guide ownership between atlas macro guidance and chunk-local realization
- snapshot/query surfaces
- edit result / dirty chunk calculation
- procedural generation result expression as `ChunkData`
- block definition / texture tile lookup
- block visual-material lookup
- block exposed-surface-height lookup for meshing
- save/load byte codec
- created world manifest / created-world chunk load support
- meshing input provision
- block-grid raycast queries
- exact top-down column sampling for previews such as debug dumps and minimap overlays

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
- `AtlasStructureRegionCoord`, `AtlasStructureRegion`
- `MountainChainGraph`, `MountainSpineSegment`
- `DrainageGraph`, `RiverPathSegment`
- `ChunkGenerationProbe`, `ColumnGenerationProbe`
- `WorldEdit`, `EditResult`
- `MeshVertex`, `CpuMesh`, `RenderBounds`
- `NeighborChunks`
- `Ray3`, `RaycastHit`
- `TopdownCell`, `TopdownColumnScan`, `TopdownSurfaceRange`, `TopdownEdge`
- `CreatedWorldManifest`, `CreatedWorldStackSummary`, `CreatedWorldSource`
- `CreateWorldConfig`
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
atlas_structure_region_coord_for_atlas(coord: AtlasCoord) -> AtlasStructureRegionCoord
atlas_structure_regions_covering_area(area: AtlasArea) -> Vec<AtlasStructureRegionCoord>

WorldCore::new(meta: WorldMeta, block_registry: Arc<BlockRegistry>) -> WorldCore
WorldCore::block_registry(&self) -> &BlockRegistry
WorldCore::block_registry_handle(&self) -> Arc<BlockRegistry>
WorldCore::loaded_chunk_bounds(&self) -> Option<(ChunkCoord, ChunkCoord)>

read_created_world_manifest(root: &Path) -> Result<CreatedWorldManifest, CreatedWorldError>
load_created_world_chunk(root: &Path, coord: ChunkCoord) -> Result<ChunkData, CreatedWorldError>
detect_latest_created_world_root(base_dir: &Path) -> io::Result<Option<PathBuf>>
write_created_world_manifest(root: &Path, manifest: &CreatedWorldManifest) -> Result<(), CreatedWorldError>
save_created_world_chunk(root: &Path, chunk: &ChunkData) -> Result<PathBuf, CreatedWorldError>
summarize_created_world_stack(world: &WorldCore, center_x: i32, center_z: i32, min_chunk_y: i32, max_chunk_y: i32) -> CreatedWorldStackSummary
create_world_to_directory(root: &Path, config: CreateWorldConfig, block_registry: &BlockRegistry) -> Result<CreatedWorldManifest, CreatedWorldError>

storage::load_chunk(bytes: &[u8]) -> Result<ChunkData, StorageError>
storage::save_chunk(snapshot: &ChunkSnapshot) -> Result<Vec<u8>, StorageError>

sample_topdown_columns(
    world: &WorldCore,
    registry: &BlockRegistry,
    min_world_x: i32,
    min_world_z: i32,
    width_blocks: u32,
    height_blocks: u32,
    min_world_y: i32,
    max_world_y: i32,
) -> Vec<TopdownColumnScan>
topdown_surface_range(columns: &[TopdownColumnScan]) -> Option<TopdownSurfaceRange>
color_topdown_cell(
    cell: TopdownCell,
    registry: &BlockRegistry,
    surface_range: TopdownSurfaceRange,
) -> [u8; 3]
topdown_edge_strength_for_cell(
    columns: &[TopdownColumnScan],
    width: usize,
    height: usize,
    x: usize,
    z: usize,
    edge: TopdownEdge,
) -> f32
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
4. created-world helpers may load or write chunk bytes, but in-memory chunk ownership still belongs to `WorldCore`
5. partial-height block geometry such as lowered exposed water surfaces is decided on the world side before renderer upload
6. chunk-order-independent macro terrain direction such as mountain spines and river paths belongs to atlas/world rather than per-chunk realization code
7. atlas structure may be generated on demand by region, but the resulting guides must remain deterministic and independent of generation order
8. planned meso terrain guides must also remain deterministic, span multiple chunks, and avoid whole-world precomputation

### Submodules

- `meta.md`: `WorldMeta` seed and version contract
- `coord.md`: world/chunk/local coordinate conversion rules
- `chunk.md`: `ChunkData` / `ChunkSnapshot` structure and invariants
- `core.md`: `WorldCore` ownership and top-level API
- `edit.md`: `WorldEdit` / `EditResult` mutation contract
- `query.md`: read-only block/chunk/region/raycast surface
- `topdown.md`: exact top-down column sampling and diagnostic preview colors
- `registry.md`: block definition, texture tile, and material contract
- `generation.md`: chunk generation rules
- `atlas/atlas.md`: atlas prototype contracts
- `atlas/structure.md`: atlas-owned mountain-chain and drainage skeleton contract
- `atlas/meso.md`: planned atlas-owned multi-chunk terrain-guide contract
- `storage.md`: raw chunk byte serialization contract
- `created.md`: created-world manifest / created-world runtime load contract
- `meshing.md`: snapshot-to-CPU-mesh contract

### Current Implementation Notes

- the default block registry is loaded from `assets/blocks/index.toml`
- chunk acquisition can now come from either created-world disk load or procedural generation before converging back into the same in-memory `WorldCore`
- meshing still operates on snapshots and renderer upload still happens outside `world`
- top-down preview sampling now also stays world-owned so app minimaps and debug tools can reuse the same realized block-column interpretation rules
- exposed-water height and top-face terrace contour hints are now produced in world meshing so renderer readability effects stay anchored to world-owned geometry meaning
- atlas terrain realization is now hybrid scalar + structure-aware: atlas/world emit region-owned mountain-chain and initial drainage guides, and generation consumes them before final chunk hydrology
- atlas remains intentionally macro at the current scale; local readability and more casual multi-chunk terrain identity should come from a later meso layer rather than from shrinking atlas cells
- that future meso layer is expected to sit between atlas and generation micro detail as an on-demand deterministic guide, but the concrete feature catalog is still intentionally undecided
- the planned long-term terrain pipeline is `atlas scalar macro -> atlas structure -> atlas meso guides -> generation profile families -> local detail and smoothing -> hydrology -> material/block fill`
