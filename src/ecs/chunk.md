# chunk

## Role

- Manage ECS-owned chunk meta state.
- Track interest, acquisition, meshing, and render-ready status without owning raw chunk data.

## Owned Data

### ChunkStates
- current interest chunk set
- loaded chunk set
- load-requested chunk set
- generate-requested chunk set
- mesh-requested chunk set
- render-ready chunk set

## Inputs

- local player position
- current baked-world availability
- current world chunk presence
- jobs results

## Outputs

- `ChunkStates.interest`
- `ChunkStates.render_ready`
- `LoadChunk` / `GenerateChunk` requests
- `BuildChunkMesh` requests

## State Transition Rules

- player movement determines the center interest chunk
- the current minimal implementation expands horizontal interest to a `5x5` neighborhood around the player chunk
- when a baked world is active, interest expands that `5x5` neighborhood across the baked vertical chunk range so collision has loaded columns to work with
- if an interesting chunk is missing and the baked manifest contains it, ECS requests `LoadChunk`
- otherwise ECS falls back to `GenerateChunk`
- loaded but non-render-ready chunks on the player plane request meshing

## Invariants

- ECS owns chunk meta state only, not raw chunk storage
- load/generate/mesh request dedupe stays deterministic
- visible chunks come from the render-ready set in the current minimal slice

## Non-Responsibilities

- chunk I/O
- procedural generation algorithms
- meshing algorithms
- renderer draw submission

## Related Modules

- `player.rs`
- `camera.rs`
- `jobs.rs`
- `../world/world.md`

## Notes

- the current visible-chunk slice is still simple: it returns the render-ready set directly
- the current interest logic is intentionally broader than the first prototype because player collision now treats missing chunks as blocking
- bootstrap uses the same horizontal chunk radius so the first rendered frame already matches the steady-state acquisition envelope
