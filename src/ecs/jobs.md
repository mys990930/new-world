# jobs

## Role

- Own the ECS side of the jobs boundary.
- Interpret job results into ECS chunk meta state and derive follow-up chunk requests.

## Owned Data

### Chunk Meta Rules
- load-requested dedupe policy
- generate-requested dedupe policy
- mesh-requested dedupe policy
- loaded / render-ready transition rules

## Inputs

- `JobResult`
- `ChunkStates`
- `WorldCore`
- optional `BakedWorldSource`

## Outputs

- updated ECS chunk meta state
- deterministic job requests for the jobs system

## State Transition Rules

- chunk job planning always starts from the current interest set
- baked-world chunks prefer `LoadChunk`
- non-baked or out-of-bounds chunks fall back to `GenerateChunk`
- load/generate success marks chunks as loaded
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
- `../world/baked.md`

## Notes

- the current minimal chunk pipeline is:
  - baked path: `LoadChunk -> BuildChunkMesh`
  - fallback path: `GenerateChunk -> BuildChunkMesh`
- app still owns the actual `world.insert_chunk(...)` and `renderer.apply_upload(...)` calls
