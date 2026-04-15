# jobs

## Role

- Own the ECS side of the jobs boundary.
- Interpret job results into ECS chunk meta state and derive follow-up chunk requests.

## Owned Data

### Chunk Meta Rules
- load-requested dedupe policy
- generate-requested dedupe policy
- mesh-requested dedupe policy
- remesh-needed invalidation policy
- loaded / render-ready transition rules

## Inputs

- `JobResult`
- `ChunkStates`
- `WorldCore`
- optional `CreatedWorldSource`

## Outputs

- updated ECS chunk meta state
- deterministic job requests for the jobs system

## State Transition Rules

- chunk job planning always starts from the current interest set
- created-world chunks prefer `LoadChunk`
- non-created or out-of-bounds chunks fall back to `GenerateChunk`
- load/generate success marks chunks as loaded
- load/generate success also invalidates already-loaded adjacent chunk meshes so seam-sensitive terrain can rebuild against the new neighbor snapshot
- mesh success marks chunks as render-ready

## Invariants

- ECS does not execute jobs directly
- world chunk insertion still happens in app after jobs complete
- ECS chunk meta state must stay consistent with applied job results

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
- the current steady-state horizontal interest envelope is a fixed `5x5` neighborhood around the focused player chunk
- app still owns the actual `world.insert_chunk(...)` and `renderer.apply_upload(...)` calls
