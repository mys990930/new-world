# bridge_ui

## Role

- convert app-owned HUD, menu, minimap, and overlay layout into renderer-owned UI sprites

## Responsibilities

- build in-game HUD sprite groups
- build the current player chunk HUD panel from the local player transform
- build inventory overlay sprites
- build minimap overlay sprites from cached minimap viewport data
- build the minimap-adjacent local environment status panel from ECS snapshots
- build world-select screen sprites from app-owned layout state
- build the world-select loading popup progress bar and counter label from app-owned layout geometry
- keep shared pixel-atlas panel / text helper functions in one place
- emit sprites against `assets/ui/new_world_pixel_ui_atlas.png`, the generated 16x20 pixel UI atlas used by the main menu, overlays, build quickslot block icons, and tool quickslot icons

## Inputs

- `PlayerInventory`
- local player `Transform`
- `LocalEnvironmentSnapshot`
- `AppMinimapViewport`
- `WorldSelectLayout`
- world-select popup progress label text prepared by app UI layout
- current viewport size
- block registry data needed for minimap colors and inventory labels
- block registry data needed for minimap colors, inventory labels, and block icon selection

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
- build quickslots render block icon sprites from the lower half of the UI atlas instead of replacing gameplay/world block texture ownership
- tool quickslots render shovel/pickaxe icon sprites from the atlas instead of text labels
- current chunk presentation may derive a read-only chunk coordinate from the player transform, but it must not drive lifecycle policy
- local environment status text may read ECS snapshots, but this layer must not query `WorldCore` or simulation directly
- renderer environment weather presentation is handled by `fixed.rs`; this UI bridge only formats the ECS-local HUD snapshot and does not map chunk weather scalars into render lighting

## Related Modules

- `bridge.md`
- `minimap.md`
- `ui.md`
- `../renderer/ui.md`
