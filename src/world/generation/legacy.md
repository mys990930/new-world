# legacy

## Role

- expose the current V1 chunk generator explicitly as legacy while V2 scaffolding is built alongside it
- preserve the existing playable and testable generation path without pretending it matches the target region-first architecture

## Responsibilities

- wrap the current `generate_chunk`, probe, and LOD helpers under a clearly named legacy surface
- preserve current deterministic behavior for the active runtime
- make it explicit that the current top-level generator alias still points at V1

## Non-Responsibilities

- implementing region-first generation
- owning new V2 contracts
- changing current terrain behavior

## Public Interface

```rust
legacy::generate_chunk(coord: ChunkCoord, meta: &WorldMeta, registry: &BlockRegistry) -> ChunkData
legacy::probe_chunk(coord: ChunkCoord, meta: &WorldMeta) -> ChunkGenerationProbe
legacy::probe_column(coord: ChunkCoord, local_x: u8, local_z: u8, meta: &WorldMeta) -> ColumnGenerationProbe
legacy::sample_chunk_surface_lod(coord: ChunkCoord, step_blocks: u8, meta: &WorldMeta) -> ChunkSurfaceLodGrid
```

## Current Status

- implemented as a thin wrapper over the existing V1 realization path
- intended to remain stable while V2 scaffolding grows in parallel
