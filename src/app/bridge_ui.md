# bridge_ui

## Role

- convert app-owned HUD, menu, minimap, and overlay layout into renderer-owned UI sprites

## Responsibilities

- build in-game HUD sprite groups
- build inventory overlay sprites
- build minimap overlay sprites from cached minimap viewport data
- build the minimap-adjacent local environment status panel from ECS snapshots
- build world-select screen sprites from app-owned layout state
- keep shared pixel-atlas panel / text helper functions in one place

## Inputs

- `PlayerInventory`
- `LocalEnvironmentSnapshot`
- `AppMinimapViewport`
- `WorldSelectLayout`
- current viewport size
- block registry data needed for minimap colors and inventory labels

## Outputs

- `Vec<RenderUiSprite>`

## Boundary Rules

- layout and overlay ownership stays in `app`
- gameplay state is read-only here
- this layer emits renderer DTOs only and does not handle hit testing or world mutation

## Invariants

- UI sprite geometry must stay aligned with the app-owned layout data used for interaction
- minimap rendering must consume app-owned cached viewport data rather than scanning live world state
- inventory and quickbar presentation may read ECS inventory snapshots, but they must not own or mutate inventory state
- local environment status text may read ECS snapshots, but this layer must not query `WorldCore` or simulation directly

## Related Modules

- `bridge.md`
- `minimap.md`
- `ui.md`
- `../renderer/ui.md`
