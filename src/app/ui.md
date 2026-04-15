# ui

## Role

- Own app-level screen mode and lightweight menu / overlay interaction state.

## Responsibilities

- store the current top-level `AppMode`
- store minimap overlay visibility
- store world-select screen state such as active section, discovered created worlds, create-world draft, spawn chunk draft, and button/status feedback
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
- create-world seed and radius draft
- spawn chunk x/z draft
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
- `Tab` toggles the minimap overlay while `InGame`
- `Escape` closes `WorldSelect`
- left-clicking a spinner arrow or action button applies the matching world-select action immediately
- clicking inside a panel updates the active `WorldSelectSection` for highlight and keyboard fallback
- `CreateWorld` section: seed/radius spinner arrows adjust the create draft and the create button queues create-world work
- `SelectCreatedWorld` section: world spinner arrows change the selected created world and the load button loads it
- `SpawnChunk` section: x/z spinner arrows adjust the load chunk, reset returns to the selected created-world preview chunk, and load-at-chunk applies it
- `Up/Down`, `Left/Right`, `Q/E`, `W/S`, `Enter`, and `B` remain as keyboard fallback for the active section

## Invariants

- screen mode is app-owned rather than ECS-owned
- world-select interaction stays app-owned and does not force renderer or ECS to learn created-world manifest details
- menu actions call app-level create/reload helpers, then return to the normal `app -> ecs -> world/jobs -> renderer` runtime flow

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

- the current world-select screen is a mouse-driven three-panel layout with spinner rows and explicit action buttons
- the current usable sections are `Create World`, `Select World`, and `Spawn Chunk`, each rendered from the same app-owned layout used for hit testing
- discovered created worlds are sorted by most-recent modification time
- the current create-world flow queues a jobs request, uses app-owned defaults for vertical chunk range, and writes into `target/world-create`
