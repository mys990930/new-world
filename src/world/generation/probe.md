# probe

## Role

- Expose deterministic inspection helpers for atlas-driven chunk generation.

## Responsibilities

- sample the same atlas neighborhood used by `generate_chunk`
- report the bilerp atlas inputs for a chosen block column
- report the resolved `TerrainProfile` and final `surface_y`
- summarize an entire chunk's surface range and profile distribution

## Notes

- Probe helpers are for tuning and diagnosis only; they do not place blocks or mutate world state.
- The probe path intentionally mirrors the real `sample -> profile -> surface` pipeline so debug numbers stay aligned with generated chunks.
