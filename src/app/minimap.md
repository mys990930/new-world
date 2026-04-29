# minimap

## Role

- own app-side minimap cache state and cache invalidation policy
- keep minimap viewport composition out of `bridge.rs` and worker scheduling details out of renderer code

## Responsibilities

- store cached top-down chunk-column data keyed by chunk `x/z`
- track pending and dirty minimap rebuild columns
- remove cached chunk-column data when no loaded chunk remains in that column
- compose the current player-centered one-chunk minimap viewport from cached columns
- expose cache update helpers for completed minimap jobs
- expose lightweight diagnostic counters for cached, pending, and dirty minimap columns
- expose local single-column patch helpers for future world edits

## Non-Responsibilities

- scanning live world chunks every frame
- worker execution
- renderer sprite emission
- gameplay interaction rules

## Owned Data

### `AppMinimapCache`
- cached chunk-column patches
- pending rebuild set
- dirty-while-pending set

### `AppMinimapViewport`
- player-centered world-space viewport origin
- cached `TopdownColumnScan` cells for the current visible minimap window
- optional visible surface range derived from the cached cells

## Inputs

- loaded chunk-column snapshots from `WorldCore`
- completed minimap rebuild results from `jobs`
- optional local world-space block changes for future patching
- chunk unload notifications from app frame orchestration
- player world position when composing a render viewport

## Outputs

- jobs-facing rebuild intent for chunk columns
- render-bridge-facing cached viewport data
- app-frame-facing diagnostic counts for minimap cache pressure

## State Transition Rules

- when a chunk column changes and no rebuild is pending, that column becomes pending and should enqueue a minimap rebuild job
- when a chunk column changes while a rebuild is already pending, that column becomes dirty and must enqueue one more rebuild after the pending result arrives
- completed minimap-column results replace the cached patch for that `x/z` column
- when the last loaded chunk in a column is unloaded, that cached column should be removed instead of rebuilt
- local future world edits may patch just one cached block column instead of rebuilding the whole viewport

## Invariants

- app owns cache lifetime because minimap visibility and viewport policy are app concerns, not world concerns
- cached data is chunk-column-scoped, not frame-scoped
- viewport composition may read up to four nearby cached chunk columns when the player stands near chunk boundaries
- minimap rendering should never require a live full-window world scan during normal frames
- diagnostic counters must be read-only and must not trigger rebuilds or live world scans

## Related Modules

- `state.rs`
- `frame.rs`
- `bridge.rs`
- `../jobs/request.md`
- `../jobs/result.md`
- `../world/topdown.md`

## Notes

- the current minimap still covers a one-chunk `32x32` block window around the player
- the current runtime refreshes cached minimap columns when chunk load/generate results arrive
- chunk unload now also participates: losing one chunk in a still-loaded column should rebuild that column, while losing the last chunk in a column should drop the cached patch entirely
- opt-in frame diagnostics include minimap cache `cached/pending/dirty` counts so minimap rebuild backlog can be distinguished from mesh/load backlog
- future player block edits should patch only the affected local block columns whenever possible
