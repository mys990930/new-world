# baked

## Role

- Define runtime-facing baked-world metadata and disk-load helpers.
- Keep baked manifest / chunk-path rules inside `world`.

## Responsibilities

- baked manifest schema
- baked world root discovery
- baked root open / validation
- baked chunk file-path rules
- baked chunk decode through world storage
- baked manifest write / chunk save helpers for tooling and app-owned bake flows
- baked stack summary scoring for preview-center selection

## Non-Responsibilities

- deciding when baked worlds should be used
- job scheduling
- renderer upload
- live loaded chunk ownership

## Owned Data

- `BakedWorldManifest`
- `BakedStackSummary`
- `BakedWorldSource`
- `BakedWorldError`

## Inputs

- baked world root path
- `manifest.toml`
- baked chunk `.bin` payloads

## Outputs

- parsed manifest metadata
- validated baked-world source handles
- decoded `ChunkData`

## Public Interface
```rust
read_baked_world_manifest(root: &Path) -> Result<BakedWorldManifest, BakedWorldError>
load_baked_chunk(root: &Path, coord: ChunkCoord) -> Result<ChunkData, BakedWorldError>
detect_latest_baked_world_root(base_dir: &Path) -> io::Result<Option<PathBuf>>
write_baked_world_manifest(root: &Path, manifest: &BakedWorldManifest) -> Result<(), BakedWorldError>
save_baked_chunk(root: &Path, chunk: &ChunkData) -> Result<PathBuf, BakedWorldError>
summarize_baked_stack(world: &WorldCore, center_x: i32, center_z: i32, min_chunk_y: i32, max_chunk_y: i32) -> BakedStackSummary
bake_world_to_directory(root: &Path, config: BakeWorldConfig, block_registry: &BlockRegistry) -> Result<BakedWorldManifest, BakedWorldError>

BakedWorldSource::open(root: impl AsRef<Path>) -> Result<BakedWorldSource, BakedWorldError>
BakedWorldSource::root(&self) -> &Path
BakedWorldSource::manifest(&self) -> &BakedWorldManifest
BakedWorldSource::contains_chunk(&self, coord: ChunkCoord) -> bool
BakedWorldSource::default_preview_chunk(&self) -> ChunkCoord
BakedWorldSource::load_chunk(&self, coord: ChunkCoord) -> Result<ChunkData, BakedWorldError>
```

## Invariants

- baked manifest format version must match the runtime-supported baked format
- baked chunk decode still goes through `world::storage::load_chunk(...)`
- baked-world helpers do not own the live runtime chunk map

## Related Modules

- `core.md`
- `storage.md`
- `../jobs/request.md`
- `../jobs/routing.md`

## Notes

- the current app bootstrap policy may ask for an explicit baked world root first and only fall back to most-recent root discovery under `target/world-bake`
- app-owned tooling such as a world-select bake action may reuse the world-owned baked helpers without depending on debug binaries
- the current baked helper surface now supports both writing a baked directory and reopening it from the main runtime
- baked runtime support is intentionally separate from `storage.md` because manifest/root-discovery policy is broader than the raw chunk byte codec
