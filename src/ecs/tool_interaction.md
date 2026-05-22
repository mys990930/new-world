# tool_interaction

## Role

- own local-player tool-use cadence, transient block damage, and floating block-drop entities
- turn app-drained primary action requests into ECS-side block-damage outcomes that `app` can apply through `WorldEdit`

## Owned Data

### `ToolUseCooldown`

- `remaining_seconds`

### `BlockDamageTracker`

- transient map from `WorldBlockCoord` to `BlockDamage`

### `BlockDamage`

- `hp_remaining`
- `untouched_seconds`

### `FloatingBlockDrop`

- dropped block id and count
- stable base center
- age used for floating presentation

## Inputs

- `PlayerCommand::PrimaryAction` drained by `app`
- `SelectionState.interaction_preview_blocks`
- selected tool and manipulation mode from `PlayerInventory`
- loaded block data from `WorldCore`
- local-player transform/body for pickup proximity
- `FrameDeltaSeconds`

## Outputs

- `ToolActionOutcome` containing touched and broken blocks
- ECS-owned floating block-drop entities
- inventory insertion when the player moves within pickup radius
- render snapshots for `app::bridge_scene`

## State Rules

- holding left click emits repeated primary-action commands through `input.rs` while a tool is selected in interaction mode
- the prototype global tool-use cooldown is `0.5` seconds for every tool
- each tool use deals `1` damage to each selected target block
- prototype block HP is `3` for every block
- damaged block HP recovers by removing its transient damage entry after `5` untouched seconds
- damaged block HP also recovers when the owning chunk is no longer loaded
- when HP reaches zero, ECS reports a break outcome; `app` applies the actual `WorldEdit::SetBlock { block: AIR }`
- successful app-side block destruction spawns one floating block drop at the destroyed block
- floating block drops are picked up when the local player is within `0.5` blocks of the drop anchor
- picked-up blocks are inserted into block quickslots first, then general inventory slots

## Tuning Notes

- tool/block efficiency, block HP, damage values, pickup radius, and the `0.5` second tool cooldown are prototype constants and should become data-driven later
- shovel currently damages the selected face-plane preview set, while pickaxe damages the single selected block because selection shape still comes from `ToolCatalog`

## Invariants

- ECS owns transient gameplay state, but world source-of-truth block mutation stays in `world`
- app coordinates break edits only after ECS produces a deterministic outcome
- dirty block-damage state stays bounded to recently touched, loaded blocks
- floating drops are ECS entities, not renderer-owned particles
- inventory insertion never creates tool stacks and only inserts block drops

## Non-Responsibilities

- raw mouse input capture
- applying `WorldEdit` directly
- renderer mesh invalidation
- GPU sprite or mesh creation

## Related Modules

- `input.rs`
- `inventory.rs`
- `selection.rs`
- `runtime.rs`
- `../app/frame.md`
- `../app/bridge_scene.md`
