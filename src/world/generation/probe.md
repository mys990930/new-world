# probe

## Role

- Expose deterministic inspection helpers for atlas-driven chunk generation.

## Responsibilities

- sample the same atlas neighborhood used by `generate_chunk`
- report the bilerp atlas inputs for a chosen block column
- report the resolved `TerrainProfile` and smoothed final `surface_y`
- summarize an entire chunk's surface range and profile distribution
- expose coarse `step_blocks` surface sampling so wide debug previews can inspect height scaffolds without materializing every block mesh

## Notes

- Probe helpers are for tuning and diagnosis only; they do not place blocks or mutate world state.
- The probe path intentionally mirrors the real `sample -> dominant profile -> smoothed surface field` pipeline so debug numbers stay aligned with generated chunks.
- `sample_chunk_surface_lod(...)` is intended for wide-angle debug rendering: it samples one representative column per coarse surface cell, preserving world-space continuity while reducing render density.
