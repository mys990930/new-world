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
- deterministic region-classification ownership between atlas raw fields / skeleton guidance and chunk-local realization
- deterministic meso terrain-guide ownership after region classification and before final chunk-local realization
- surface material policy and seasonal biome-state ownership before final voxel fill
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
- snapshot-based top-down chunk-column derivation for cached minimap rebuild jobs

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
- `RegionClassMap`, `RegionClassSample`
- `TemperatureBand`, `MoistureBand`, `ElevationBand`
- `ReliefClass`, `HydrologyContext`, `CoastalContext`, `ClimateRegime`
- `BiomeFamily`, `TerrainFormFamily`, `RegionArchetype`
- `RegionArchetypeDef`, `MesoFeatureDef`
- `AtlasStructureRegionCoord`, `AtlasStructureRegion`
- `MountainChainGraph`, `MountainSpineSegment`
- `DrainageGraph`, `RiverPathSegment`
- `MesoGuideMap`, `MesoGuideCell`, `MesoGuideSample`
- `MesoRegionCoord`, `MesoRegion`
- `ChunkGenerationProbe`, `ColumnGenerationProbe`
- `WorldEdit`, `EditResult`
- `MeshVertex`, `CpuMesh`, `RenderBounds`
- `NeighborChunks`
- `Ray3`, `RaycastHit`
- `TopdownCell`, `TopdownColumnScan`, `TopdownSurfaceRange`, `TopdownEdge`
- `MaterialPolicyDef`, `MaterialPolicyId`
- `SeasonalBiomeStateDef`, `SeasonalBiomeStateId`, `SeasonalPhase`
- `CoverOverrideRule`, `CoverPhase`
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
resolve_region_classes(
    meta: &WorldMeta,
    area: AtlasArea,
    fields: &AtlasFieldMap,
    structure: &AtlasStructureMap,
) -> RegionClassMap
sample_region_classes(
    classes: &RegionClassMap,
    world_x: i32,
    world_z: i32,
) -> RegionClassSample
region_archetype_defs() -> &'static [RegionArchetypeDef]
region_archetype_def(id: RegionArchetype) -> Option<&'static RegionArchetypeDef>
meso_feature_defs() -> &'static [MesoFeatureDef]
meso_feature_def(key: &str) -> Option<&'static MesoFeatureDef>
default_material_policies() -> &'static [MaterialPolicyDef]
default_seasonal_biome_states() -> &'static [SeasonalBiomeStateDef]
default_cover_override_rules() -> &'static [CoverOverrideRule]
generate_meso_guides(
    meta: &WorldMeta,
    area: AtlasArea,
    fields: &AtlasFieldMap,
    structure: &AtlasStructureMap,
) -> MesoGuideMap
sample_meso_guides(
    guides: &MesoGuideMap,
    world_x: i32,
    world_z: i32,
) -> MesoGuideSample
atlas_structure_region_coord_for_atlas(coord: AtlasCoord) -> AtlasStructureRegionCoord
atlas_structure_regions_covering_area(area: AtlasArea) -> Vec<AtlasStructureRegionCoord>
meso_region_coord_for_atlas(coord: AtlasCoord) -> MesoRegionCoord
meso_regions_covering_area(area: AtlasArea) -> Vec<MesoRegionCoord>

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
sample_single_topdown_column(
    world: &WorldCore,
    registry: &BlockRegistry,
    world_x: i32,
    world_z: i32,
    min_world_y: i32,
    max_world_y: i32,
) -> TopdownColumnScan
sample_topdown_chunk_column(
    registry: &BlockRegistry,
    coord: TopdownChunkColumnCoord,
    chunks: &[ChunkSnapshot],
) -> TopdownChunkColumnPatch
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
8. atlas-owned meso terrain guides must remain deterministic, span multiple chunks, and avoid whole-world precomputation

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
- `atlas/region.md`: atlas-owned region classification contract
- `atlas/region_catalog.md`: authoritative launch catalog for biome, terrain-form, archetype, and meso planning
- `atlas/region/axes.md`: classification dimension reference
- `atlas/region/catalog.md`: scaffolded archetype catalog index
- `atlas/region/archetypes/archetypes.md`: per-archetype module/doc structure
- `atlas/structure.md`: atlas-owned mountain-chain and drainage skeleton contract
- `atlas/meso.md`: atlas-owned multi-chunk terrain-guide contract
- `atlas/meso/catalog.md`: scaffolded meso feature catalog index
- `atlas/meso/features/features.md`: per-feature module/doc structure
- `surface/surface.md`: material, cover, and seasonal surface policy contract
- `storage.md`: raw chunk byte serialization contract
- `created.md`: created-world manifest / created-world runtime load contract
- `meshing.md`: snapshot-to-CPU-mesh contract

### Current Implementation Notes

- the default block registry is loaded from `assets/blocks/index.toml`
- chunk acquisition can now come from either created-world disk load or procedural generation before converging back into the same in-memory `WorldCore`
- meshing still operates on snapshots and renderer upload still happens outside `world`
- top-down preview sampling now also stays world-owned so app minimaps and debug tools can reuse the same realized block-column interpretation rules
- the current minimap cache flow uses snapshot-based top-down chunk-column derivation in jobs, while render-time viewport composition stays app-owned
- exposed-water height and top-face terrace contour hints are now produced in world meshing so renderer readability effects stay anchored to world-owned geometry meaning
- atlas terrain realization is now hybrid scalar + structure-aware: atlas/world emit region-owned mountain-chain and initial drainage guides, and generation consumes them before final chunk hydrology
- atlas remains intentionally macro at the current scale; local readability and more casual multi-chunk terrain identity should come from a later meso layer rather than from shrinking atlas cells
- the next authoritative ownership step is atlas-owned region classification, which should resolve biome and terrain-form archetypes before base heightfield solving
- an initial code scaffold for atlas-owned region classification now exists, but it is not yet the active gameplay generator path
- the current public `resolve_region_classes(...)` surface remains launch-safe by applying launch fallback at the final step, while the internal raw classifier may still emit broader scaffolded archetypes for testing and future downstream work
- atlas-owned meso guides should eventually sit after region classification and before generation micro detail as on-demand deterministic terrain accents
- the full per-feature meso taxonomy is now scaffolded in `atlas/meso/features/*`, but the currently emitted runtime subset is still only the Wave 1A guides: `hill clusters`, `basins`, `escarpment bands`, and `terraces`
- the old legacy V1 chunk generator has now been removed instead of being kept beside V2
- the current top-level `world::generate_chunk(...)` path is an explicit TODO stub until V2 realization lands
- the current `probe_chunk(...)`, `probe_column(...)`, and `sample_chunk_surface_lod(...)` surfaces are also compile-only TODO stubs
- the planned long-term terrain pipeline is `atlas raw fields -> atlas skeleton -> region classification -> realization field solve -> river corridor solve -> biome-aware base heightfield -> meso accents -> final hydrology -> material/block fill`
