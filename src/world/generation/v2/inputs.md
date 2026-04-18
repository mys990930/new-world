# inputs

## Role

- hold the first assembled atlas, skeleton, region-classification, and corridor handoff inputs for V2 chunk generation

## Current Types

- `ChunkGenerationV2Inputs`
- `ChunkGenerationV2Scaffold`
- `V2ScaffoldStage`

## Notes

- this module now stops at the corridor-window-ready checkpoint
- `ChunkGenerationV2Inputs` now also carries a copied `WorldMeta` so later continuity-aware prototype work can lazily gather neighboring chunk state without widening the public prototype interface
- `ChunkGenerationV2Scaffold` carries the corridor window forward so prototype can consume it explicitly
- the next stage boundary is the base-heightfield prototype solve documented in `v2/prototype.md`
