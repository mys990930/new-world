# created

## Role

- Define runtime-facing created-world metadata and disk-load helpers.
- Keep created-world manifest / chunk-path rules inside `world`.

## Responsibilities

- created-world manifest schema
- created world root discovery
- created-world root open / validation
- created-world chunk file-path rules
- created-world chunk decode through world storage
- created-world manifest write / chunk save helpers for tooling and app-owned create-world flows
- created-world stack summary scoring for preview-center selection

## Non-Responsibilities

- deciding when created worlds should be used
- job scheduling
- renderer upload
- live loaded chunk ownership

## Owned Data

- `CreatedWorldManifest`
- `CreatedWorldStackSummary`
- `CreatedWorldSource`
- `CreateWorldProgress`
- `CreatedWorldError`

## Inputs

- created world root path
- `manifest.toml`
- created-world chunk `.bin` payloads

## Outputs

- parsed manifest metadata
- validated created-world source handles
- decoded `ChunkData`

## Public Interface
```rust
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

CreatedWorldSource::open(root: impl AsRef<Path>) -> Result<CreatedWorldSource, CreatedWorldError>
CreatedWorldSource::root(&self) -> &Path
CreatedWorldSource::manifest(&self) -> &CreatedWorldManifest
CreatedWorldSource::contains_chunk(&self, coord: ChunkCoord) -> bool
CreatedWorldSource::default_preview_chunk(&self) -> ChunkCoord
CreatedWorldSource::load_chunk(&self, coord: ChunkCoord) -> Result<ChunkData, CreatedWorldError>
```

## Invariants

- created-world manifest format version must match the runtime-supported created-world format
- created-world chunk decode still goes through `world::storage::load_chunk(...)`
- created-world helpers do not own the live runtime chunk map
- create-world progress reports count chunk generation/write completion only; manifest write completion is represented by the final success result

## Related Modules

- `core.md`
- `storage.md`
- `../jobs/request.md`
- `../jobs/routing.md`

## Notes

- the current app bootstrap policy may ask for an explicit created world root first and only fall back to most-recent root discovery under `target/world-create`
- app-owned tooling such as a world-select create-world action may reuse the world-owned created-world helpers without depending on debug binaries
- the current created-world helper surface now supports both writing a created-world directory and reopening it from the main runtime
- the progress-aware create helper emits `completed_chunks / total_chunks` snapshots so async callers can display determinate feedback without taking ownership of world generation
- the current `CreateWorldConfig::default()` uses radius `0` to keep quick-start world creation to a single `x/z` stack; larger worlds should be requested explicitly by tooling or UI
- created-world runtime support is intentionally separate from `storage.md` because manifest/root-discovery policy is broader than the raw chunk byte codec
- created-world generation should solve expensive generation surface / voxelization-plan work once per `x/z` stack, reuse cached generation input bundles by generation atlas area, and only repeat final y-specific voxel writes per vertical chunk
- independent `x/z` stacks may generate in parallel because they write disjoint chunk paths and converge through deterministic manifest sorting
- stack summary scoring should read the same voxelization plan used for block fill rather than building a temporary `WorldCore` solely to rescan blocks
