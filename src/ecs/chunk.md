# chunk

## Role

- Manage ECS-owned chunk meta state.
- Track interest, acquisition, meshing, and render-ready status without owning raw chunk data.

## Owned Data

### ChunkStates
- current interest chunk set
- current retain chunk set
- loaded chunk set
- load-requested chunk set
- generate-requested chunk set
- unload-requested chunk set
- mesh-requested chunk set
- remesh-needed chunk set
- render-ready chunk set

### ChunkLifecyclePlan
- next interest chunk set
- next retain chunk set
- chunk job requests
- chunk unload coords

## Inputs

- local player position
- current created-world availability
- current world chunk presence
- jobs results

## Outputs

- `ChunkStates.interest`
- `ChunkStates.retain`
- `ChunkStates.render_ready`
- `LoadChunk` / `GenerateChunk` requests
- `UnloadChunk` requests
- `BuildChunkMesh` requests
- unload coord list

## State Transition Rules

- player movement determines the center interest chunk
- the current minimal implementation expands horizontal interest to a `7x7` neighborhood around the player chunk
- the steady-state retain envelope expands horizontal retention to a `9x9` neighborhood around the player chunk
- when a created world is active, interest expands that `7x7` neighborhood across the created-world vertical chunk range so collision has loaded columns to work with
- when a created world is active, retain expands that broader horizontal neighborhood across the created-world vertical chunk range too
- if an interesting chunk is missing and the created-world manifest contains it, ECS requests `LoadChunk`
- otherwise ECS falls back to `GenerateChunk`
- loaded but non-render-ready interesting chunks request meshing
- loaded interesting chunks whose boundary neighbors changed may request meshing again even if they already have a render mesh
- when created-world interest spans multiple `y` chunk layers, vertically loaded chunks must also become render-ready so lower terrain can render instead of only remaining selectable
- loaded chunks that leave `retain` become unload candidates
- unload candidates become `UnloadChunk` requests and are tracked as pending until the job result is drained
- loaded chunks outside `interest` but still inside `retain` stay resident, which avoids repeated load/unload churn at the movement boundary

## Invariants

- ECS owns chunk meta state only, not raw chunk storage
- load/generate/mesh request dedupe stays deterministic
- chunk-boundary mesh refresh stays ECS-owned meta state rather than living in renderer/world upload bookkeeping
- visible chunks come from the render-ready set in the current minimal slice, so interest chunks that should render must first pass through the mesh-request path regardless of vertical layer
- unload planning and pending-unload dedupe are ECS-owned meta policy, but actual `WorldCore` removal and renderer mesh removal still happen in app after the unload job result is drained
- chunks outside `retain` are no longer valid runtime targets for late job results

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
- when a chunk finishes loading or generation, already-loaded adjacent chunks are marked for remesh so contour/visibility at chunk seams can refresh against the new neighbor snapshot
- the current hysteresis plan uses `interest radius = 3` and `retain radius = 4`, giving a one-ring buffer before unload starts while showing a broader active chunk neighborhood
