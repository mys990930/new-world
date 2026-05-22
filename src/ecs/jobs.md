# jobs

## Role

- Own the ECS side of the jobs boundary.
- Interpret job results into ECS chunk meta state and derive follow-up chunk requests.

## Owned Data

### Chunk Meta Rules
- load-requested dedupe policy
- generate-requested dedupe policy
- unload-requested dedupe policy
- mesh-requested dedupe policy
- remesh-needed invalidation policy
- loaded / render-ready transition rules
- retain-envelope unload policy
- stale-result acceptance policy

## Inputs

- `JobResult`
- `ChunkStates`
- `WorldCore`
- optional `CreatedWorldSource`

## Outputs

- updated ECS chunk meta state
- deterministic job requests for the jobs system
- deterministic unload job requests for app-owned world/renderer removal after completion

## State Transition Rules

- chunk lifecycle planning starts from the current interest and retain sets
- acquisition and mesh requests are emitted in a closest-to-focus order so the spawn/current column begins streaming before outer interest chunks
- acquisition and mesh requests are capped per frame so initial streaming can progress aggressively without submitting the full interest set at once
- created-world chunks prefer `LoadChunk`
- non-created or out-of-bounds chunks fall back to `GenerateChunk`
- load/generate success marks chunks as loaded
- load/generate success also invalidates already-loaded adjacent chunk meshes so seam-sensitive terrain can rebuild against the new neighbor snapshot
- mesh requests wait while directly adjacent interest chunks are still unresolved, reducing repeated remesh churn while a neighborhood is actively streaming in
- mesh success marks chunks as render-ready
- chunks outside `retain` become `UnloadChunk` candidates even if they were previously loaded/render-ready
- pending unload requests are tracked in chunk meta state so repeated lifecycle plans do not enqueue duplicate unloads
- `ChunkUnloaded` clears unload-requested state; app then applies `world.remove_chunk(...)`, renderer mesh removal, minimap invalidation, and ECS chunk cleanup
- job results for chunks that are no longer retained must clear dedupe state but must not resurrect runtime world/render state
- create-world progress results are ignored by ECS because they carry app/UI status, not chunk lifecycle state
- region-classification results are ignored by ECS because they update app/world environment cache, not chunk lifecycle state

## Invariants

- ECS does not execute jobs directly
- world chunk insertion still happens in app after jobs complete
- ECS chunk meta state must stay consistent with applied job results
- per-frame request caps must preserve dedupe state so deferred chunks remain eligible in later lifecycle plans
- non-chunk progress or cache results must not mutate chunk meta state
- unload hysteresis is expressed through ECS-owned retain state, not through app-owned ad hoc distance checks

## Non-Responsibilities

- thread-pool execution
- file I/O
- meshing implementation
- simulation calculations

## Related Modules

- `chunk.rs`
- `fixed.rs`
- `../jobs/jobs.md`
- `../world/created.md`

## Notes

- the current minimal chunk pipeline is:
  - created-world path: `LoadChunk -> BuildChunkMesh`
  - fallback path: `GenerateChunk -> BuildChunkMesh`
- seam-sensitive terrain such as contour hints may require a second `BuildChunkMesh` pass for adjacent chunks after a neighbor chunk becomes available
- the current steady-state horizontal interest envelope is a fixed `7x7` neighborhood around the focused player chunk
- the current lifecycle emits up to 8 chunk acquisition requests and up to 4 mesh requests per frame, relying on the jobs queue and worker count to smooth actual execution
- interest-neighbor deferral means initial visual readiness may wait briefly for nearby chunks, but avoids immediately rebuilding the same mesh for every neighbor arrival
- app still owns the actual `world.insert_chunk(...)`, `world.remove_chunk(...)`, renderer mesh upload, and renderer mesh removal calls; unload jobs only move removal into the budgeted result path
- created-world load no longer depends on app-side synchronous preload; this lifecycle is responsible for turning the staged player x/z into disk `LoadChunk` work
