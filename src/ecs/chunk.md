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
- current created-world availability
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
- when a created world is active, interest expands that `5x5` neighborhood across the created-world vertical chunk range so collision has loaded columns to work with
- if an interesting chunk is missing and the created-world manifest contains it, ECS requests `LoadChunk`
- otherwise ECS falls back to `GenerateChunk`
- loaded but non-render-ready interesting chunks request meshing
- when created-world interest spans multiple `y` chunk layers, vertically loaded chunks must also become render-ready so lower terrain can render instead of only remaining selectable

## Invariants

- ECS owns chunk meta state only, not raw chunk storage
- load/generate/mesh request dedupe stays deterministic
- visible chunks come from the render-ready set in the current minimal slice, so interest chunks that should render must first pass through the mesh-request path regardless of vertical layer

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
