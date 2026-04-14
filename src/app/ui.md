# ui

## Role

- Own app-level screen mode and lightweight menu / overlay interaction state.

## Responsibilities

- store the current top-level `AppMode`
- store minimap overlay visibility
- store world-select screen state such as active section, discovered baked worlds, bake draft, and spawn chunk draft
- handle screen-level shortcuts that should not belong to ECS
- trigger app-owned bake / baked-world reload helpers without moving ownership into ECS or renderer

## Owned Data

### AppMode
- `InGame`
- `WorldSelect`

### WorldSelectSection
- `Bake`
- `Bakes`
- `SpawnChunk`

### BakedWorldOption
- baked root path
- display label
- parsed `BakedWorldManifest`

### WorldSelectState
- active `WorldSelectSection`
- discovered baked-world list
- selected baked-world index
- bake seed and radius draft
- spawn chunk x/z draft
- status line

### AppUiState
- current `AppMode`
- minimap overlay visibility flag
- `WorldSelectState`

## Inputs

- platform raw input snapshot as read by `app`
- current baked-world runtime root
- baked-world manifests discovered under the configured baked-world directory

## Outputs

- app-mode transitions
- overlay toggles
- app-owned bake requests
- app-owned baked-world reload requests

## State Transition Rules

- `F1` toggles between `InGame` and `WorldSelect`
- `Tab` toggles the minimap overlay while `InGame`
- `Escape` closes `WorldSelect`
- `Up/Down` changes the active world-select section
- `Bake` section: `Left/Right` changes bake radius and `Enter` or `B` runs bake
- `Bakes` section: `Left/Right` changes the selected baked world and `Enter` loads it
- `SpawnChunk` section: `Left/Right` adjusts chunk x, `Q/E` or `W/S` adjusts chunk z, `R` resets to the selected bake preview chunk, and `Enter` loads

## Invariants

- screen mode is app-owned rather than ECS-owned
- world-select interaction stays app-owned and does not force renderer or ECS to learn baked-world manifest details
- menu actions call app-level bake/reload helpers, then return to the normal `app -> ecs -> world/jobs -> renderer` runtime flow

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

- the current world-select screen is keyboard-driven and intentionally simple
- the current usable sections are `Bake`, `Select Bake`, and `Spawn Chunk`
- discovered baked worlds are sorted by most-recent modification time
- the current bake flow uses app-owned defaults for vertical chunk range and writes into `target/world-bake`
