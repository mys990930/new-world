# ui

## Role

- Own app-level screen mode and lightweight menu / overlay interaction state.

## Responsibilities

- store the current top-level `AppMode`
- store minimap overlay visibility
- store world-select screen state such as active section, discovered created worlds, create-world draft, selected-world list scroll, spawn chunk draft, focused input field buffer, pending job popup state, and button/status feedback
- handle screen-level shortcuts that should not belong to ECS
- build and hit-test app-owned world-select layout state for mouse interaction
- trigger app-owned create-world requests / created-world reload helpers without moving ownership into ECS or renderer

## Owned Data

### AppMode
- `InGame`
- `WorldSelect`

### WorldSelectSection
- `CreateWorld`
- `SelectCreatedWorld`
- `SpawnChunk`

### CreatedWorldOption
- created-world root path
- display label
- parsed `CreatedWorldManifest`

### WorldSelectState
- active `WorldSelectSection`
- discovered created-world list
- selected created-world index
- selected created-world list scroll offset
- create-world seed, radius, and center-x/center-z draft
- spawn chunk x/z draft
- focused numeric input field and text buffer
- optional pending create-world job descriptor with chunk progress counters for loading feedback
- status line

### AppUiState
- current `AppMode`
- minimap overlay visibility flag
- `WorldSelectState`

## Inputs

- platform raw input snapshot as read by `app`
- current created-world runtime root
- created-world manifests discovered under the configured created-world directory
- current window size for world-select hit testing

## Outputs

- app-mode transitions
- overlay toggles
- app-owned create-world requests
- app-owned created-world reload requests

## State Transition Rules

- `F1` toggles between `InGame` and `WorldSelect`
- `M` toggles the minimap overlay while `InGame`
- `Escape` closes `WorldSelect`
- when the app starts without a loaded created world, `WorldSelect` is the startup screen and cannot be closed until a created world is loaded
- left-clicking a numeric value focuses that input field so typed digits from text input, number-row keys, or numpad keys write into an app-owned text buffer until commit
- left-clicking a spinner arrow or action button applies the matching world-select action immediately after committing any focused input field
- scrolling while the cursor is inside `SelectCreatedWorld` scrolls the created-world list viewport
- clicking a created-world row selects it and keeps the load button enablement app-owned
- `CreateWorld` section: seed/radius/center-x/center-z fields are editable by typing or arrows, and the create button queues background create-world work
- `SelectCreatedWorld` section: the section owns a scrollable list of discovered worlds plus the selected-world metadata and load button
- `SpawnChunk` section: x/z fields are editable by typing or arrows, reset returns to the selected created-world preview chunk, and load-at-chunk applies it
- while a create-world job is pending, world-select interaction is locked behind an app-owned loading popup; the progress bar is indeterminate until the first progress event and determinate after `completed_chunks / total_chunks` is known
- `Up/Down`, `Left/Right`, `Q/E`, `W/S`, `Enter`, and `B` remain as keyboard fallback when no input field is focused and no loading popup is active

## Invariants

- screen mode is app-owned rather than ECS-owned
- world-select interaction stays app-owned and does not force renderer or ECS to learn created-world manifest details
- menu actions call app-level create/reload helpers, then return to the normal `app -> ecs -> world/jobs -> renderer` runtime flow
- in-game HUD and inventory overlays may read ECS inventory and transform snapshots through `bridge`, but they do not own player inventory state or chunk lifecycle policy

## Non-Responsibilities

- gameplay input interpretation
- renderer draw encoding
- worker execution details

## Related Modules

- `state.rs`
- `bootstrap.rs`
- `frame.rs`
- `bridge.rs`

## Notes

- the current world-select screen is a mouse-driven three-panel layout with editable numeric fields, a scrollable created-world list, explicit action buttons, and a blocking loading popup with a progress bar for pending create-world jobs
- pending create-world progress is stored only in app UI state and is updated from jobs progress results keyed by created-world root path
- the same world-select layout acts as the no-created-world startup screen for the main binary
- the current usable sections are `Create World`, `Select World`, and `Spawn Chunk`, each rendered from the same app-owned layout used for hit testing
- discovered created worlds are sorted by most-recent modification time
- the current create-world flow queues a jobs request, uses app-owned defaults for vertical chunk range, and writes into `target/world-create`
- the default create-world radius is `0` so quick-start generation writes only one vertical stack by default; users can increase radius explicitly when they want a broader precreated area
- in-game bottom HUD and inventory window are intentionally not app-owned screen modes; they are render-only projections of ECS player inventory/transform state
- the current minimap overlay is still app-owned UI state, but its content is now a world-derived one-chunk top-down preview centered on the player and emitted through atlas-backed UI sprites
