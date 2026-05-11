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
- artifact-suppression ownership across generation handoff points so atlas, meso, and structure cache boundaries do not become visible terrain or material masks
- world calendar, date, and season source-of-truth ownership
- atlas-cell runtime climate state ownership
- deferred seasonal/weather/ecology patch ownership for far-away regions
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
- runtime region-classification sample cache for main-thread environment consumers
- deterministic climate-tree voxel blueprint generation for later generator placement

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
- `TreeKind`, `TreeBlockPalette`, `TreeGenRequest`, `TreeVoxel`, `TreeBlueprint`
- `MaterialPolicyDef`, `MaterialPolicyId`
- `SeasonalBiomeStateDef`, `SeasonalBiomeStateId`, `SeasonalPhase`
- `CoverOverrideRule`, `CoverPhase`
- `WorldCalendar`
- `AtlasClimateRuntimeState`
- `LocalWeatherState`
- `ChunkWeatherKind`, `ChunkWeatherState`, `ChunkWeatherUpdate`, `WeatherApplyResult`
- `DeferredSeasonPatch`
- `CreatedWorldManifest`, `CreatedWorldStackSummary`, `CreatedWorldSource`
- `CreateWorldProgress`
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
WorldCore::calendar(&self) -> &WorldCalendar
WorldCore::climate_state(coord: AtlasCoord) -> AtlasClimateRuntimeState
WorldCore::local_weather(coord: AtlasCoord) -> Option<LocalWeatherState>
WorldCore::chunk_weather(coord: ChunkCoord) -> Option<ChunkWeatherState>
WorldCore::set_chunk_weather(coord: ChunkCoord, state: ChunkWeatherState) -> WeatherApplyResult
WorldCore::apply_chunk_weather_update(update: ChunkWeatherUpdate) -> WeatherApplyResult
WorldCore::deferred_season_patches(&self) -> &[DeferredSeasonPatch]
WorldCore::apply_calendar_advance(advance: CalendarAdvance) -> CalendarApplyResult
WorldCore::resolve_region_class_area(area: AtlasArea) -> RegionClassMap
WorldCore::cached_region_class_area(area: AtlasArea) -> Option<RegionClassMap>
WorldCore::sample_cached_region_class_atlas(coord: AtlasCoord) -> Option<RegionClassSample>
WorldCore::sample_region_class_atlas(coord: AtlasCoord) -> RegionClassSample
WorldCore::cache_region_class_map(classes: &RegionClassMap)

read_created_world_manifest(root: &Path) -> Result<CreatedWorldManifest, CreatedWorldError>
load_created_world_chunk(root: &Path, coord: ChunkCoord) -> Result<ChunkData, CreatedWorldError>
detect_latest_created_world_root(base_dir: &Path) -> io::Result<Option<PathBuf>>
write_created_world_manifest(root: &Path, manifest: &CreatedWorldManifest) -> Result<(), CreatedWorldError>
save_created_world_chunk(root: &Path, chunk: &ChunkData) -> Result<PathBuf, CreatedWorldError>
summarize_created_world_stack(world: &WorldCore, center_x: i32, center_z: i32, min_chunk_y: i32, max_chunk_y: i32) -> CreatedWorldStackSummary
summarize_created_world_stack_from_voxelization_plan(plan: &VoxelizationPlan, center_x: i32, center_z: i32, min_chunk_y: i32, max_chunk_y: i32) -> CreatedWorldStackSummary
create_world_to_directory(root: &Path, config: CreateWorldConfig, block_registry: &BlockRegistry) -> Result<CreatedWorldManifest, CreatedWorldError>
create_world_to_directory_with_progress(root: &Path, config: CreateWorldConfig, block_registry: &BlockRegistry, report_progress: impl FnMut(CreateWorldProgress)) -> Result<CreatedWorldManifest, CreatedWorldError>
CreateWorldConfig::total_chunk_count(self) -> Option<u32>

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

TreeKind::from_key(key: &str) -> Option<TreeKind>
TreeKind::key(self) -> &'static str
TreeKind::all() -> &'static [TreeKind]
TreeBlockPalette::resolve_default(kind: TreeKind, registry: &BlockRegistry) -> Result<TreeBlockPalette, TreePaletteError>
generate_tree_blueprint(request: TreeGenRequest) -> TreeBlueprint
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
9. atlas cells, meso guide cells, structure regions, and raw skeleton segments are not visible output primitives; downstream generation must diffuse, warp, resolve, or mask them before height, hydrology, or material block decisions reach `ChunkData`
10. frame-time environment/HUD consumers must prefer cached region classification and avoid forcing atlas structure/classification resolution on the main thread
11. tree generation emits relative voxel blueprints only and does not mutate live `WorldCore` storage

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
- `surface/resolve.md`: chunk-column surface-plan resolve contract
- `tree.md`: deterministic per-climate tree voxel blueprint contract
- `calendar.md`: world-owned calendar, runtime climate, and deferred seasonal patch contract
- `weather.md`: chunk weather scalar state contract and query/apply bridge
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
- planned time/season ownership also stays world-owned: calendar, active climate drift state, and deferred far-region seasonal patches should remain world truth even when only a small active region is simulated eagerly
- the first runtime slice now implements that ownership directly in `WorldCore`: calendar/climate/weather state is no longer spec-only, and simulation feeds it through `CalendarAdvance`
- chunk-scoped weather is stored separately from old atlas `LocalWeatherState` and exposed through `WorldCore::chunk_weather(...)`, `WorldCore::set_chunk_weather(...)`, and `WorldCore::apply_chunk_weather_update(...)`
- atlas terrain realization is now hybrid scalar + structure-aware: atlas/world emit region-owned mountain-chain and initial drainage guides, and generation consumes them before final chunk hydrology
- atlas cells now use a denser `128m / 8 x 8 chunk / 1 region` footprint so terrain identity, climate, and region classification change more often during normal play
- local readability and casual multi-chunk terrain identity still also come from the meso layer, which refines atlas-owned regional identity rather than replacing it
- atlas-owned region classification now resolves biome and terrain-form archetypes before base heightfield solving
- generation-side realization-field sampling now sits after region classification so prototype can read continuous control parameters instead of atlas-cell labels directly
- the current public `resolve_region_classes(...)` surface remains launch-safe by applying launch fallback at the final step, while the internal raw classifier may still emit broader scaffolded archetypes for testing and future downstream work
- atlas-owned meso guides should eventually sit after region classification and before generation micro detail as on-demand deterministic terrain accents
- the full per-feature meso taxonomy is now scaffolded in `atlas/meso/features/*`
- atlas-owned meso guide emission is still the broad Wave 1A channel set: `hill clusters`, `basins`, `escarpment bands`, and `terraces`
- the current chunk-side runtime-backed meso subset now applies feature-owned surface resolvers for `hill_cluster`, `shallow_basin`, `escarpment_band`, `upland_terrace`, `ravine`, `coastal_cliff_band`, `dune_field`, and `crater`
- `ravine`, `coastal_cliff_band`, `dune_field`, and `crater` currently borrow those existing Wave 1A guide channels provisionally until dedicated atlas meso channels are added
- removed legacy chunk-generation code is not kept beside the active generation path
- the current top-level `world::generate_chunk(...)` path now runs the initial end-to-end generation realization stack through surface resolve and voxel block fill
- `WorldCore` now caches resolved `RegionClassSample`s so app/ECS runtime environment refreshes can read cached atlas identity while uncached classification is resolved through jobs
- the current `probe_chunk(...)`, `probe_column(...)`, and `sample_chunk_surface_lod(...)` surfaces are also compile-only TODO stubs
- the planned long-term terrain pipeline is `atlas raw fields -> atlas skeleton -> region classification -> realization field solve -> river corridor solve -> biome-aware base heightfield -> meso accents -> smoothing/local refinement -> final hydrology -> material/block fill`
