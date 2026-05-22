# inventory

## Role

- own local-player inventory state, manipulation mode, quickslot selection, and basic tool definitions
- interpret inventory-related discrete commands without leaking UI ownership into `app`

## Owned Data

### `PlayerInventory`
- `general_slots`
- `tool_quickslots`
- `block_quickslots`
- `selected_tool_slot`
- `selected_block_slot`
- `manipulation_mode`
- `inventory_open`

### `InventorySlot`
- `item`
- `count`

### `InventoryItem`
- `Tool(ToolKind)`
- `Block(BlockId)`

### `ToolKind`
- `Shovel`
- `Pickaxe`

### `ToolSpec`
- `range_blocks`
- `preview_shape`

### `ManipulationMode`
- `Interaction`
- `Build`

### `ToolCatalog`
- built-in specs for the currently supported tools

## Inputs

- `PlayerCommandBuffer`
- local player entity id

## Outputs

- updated player inventory / mode / quickslot state
- selected-tool metadata that `selection.rs` can use for preview generation
- selected quickslot data that `app::bridge` can export into HUD / inventory UI

## State Rules

- the local player owns the inventory component
- tools are non-stackable
- block stacks may grow up to `u8::MAX`
- quickslot bars are mode-specific: `interaction` uses tools, `build` uses blocks
- the active quickslot index is stored separately for the two modes
- `Tab` toggles `ManipulationMode`
- `I` toggles `inventory_open`
- wheel cycling and `1..0` direct slot selection affect only the active mode's quickslot index
- the current player reach for both interaction and build preview is `6` blocks
- if no interaction tool is selected, ECS still falls back to a default single-block interaction preview at player reach
- build preview and placement require the selected build quickslot to contain a block stack
- successful app-owned block placement consumes one item from the selected build stack

## Invariants

- inventory ownership stays in ECS, not `app`
- app-owned HUD/menu state may render inventory data, but does not own or mutate it directly
- the current prototype does not yet support drag/drop, split stacks, or moving items between slots

## Non-Responsibilities

- raw input capture
- renderer layout or sprite emission
- actual world block edit application

## Related Modules

- `input.rs`
- `command.rs`
- `selection.rs`
- `../app/bridge.md`

## Notes

- the current initial loadout is prototype-friendly: shovel + pickaxe in the tool quickbar, plus one stack each of ten common terrain blocks in the build quickbar and general slots
- current tool preview rules are minimal and visual only: pickaxe highlights a single target block, shovel highlights a small face-oriented area, and the no-tool fallback also highlights a single target block
