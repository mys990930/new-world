# probe

## Role

- hold compile-time probe and LOD data shapes while the real V2 diagnostic path is still unimplemented

## Current Status

- `probe_chunk(...)`, `probe_column(...)`, and `sample_chunk_surface_lod(...)` are intentional TODO stubs
- the structs remain so existing callers keep compiling
- runtime diagnostic behavior should be considered disabled until V2 implementations replace these stubs

## Retained Types

- `ColumnAtlasSample`
- `TerrainProfileCounts`
- `ColumnGenerationProbe`
- `ChunkGenerationProbe`
- `ChunkSurfaceLodSample`
- `ChunkSurfaceLodGrid`
