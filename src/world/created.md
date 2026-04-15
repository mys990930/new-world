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
create_world_to_directory(root: &Path, config: CreateWorldConfig, block_registry: &BlockRegistry) -> Result<CreatedWorldManifest, CreatedWorldError>

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

## Related Modules

- `core.md`
- `storage.md`
- `../jobs/request.md`
- `../jobs/routing.md`

## Notes

- the current app bootstrap policy may ask for an explicit created world root first and only fall back to most-recent root discovery under `target/world-create`
- app-owned tooling such as a world-select create-world action may reuse the world-owned created-world helpers without depending on debug binaries
- the current created-world helper surface now supports both writing a created-world directory and reopening it from the main runtime
- created-world runtime support is intentionally separate from `storage.md` because manifest/root-discovery policy is broader than the raw chunk byte codec
