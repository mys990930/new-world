# selection

## Role

- Track which world block and face the cursor is currently hovering
- Expose the minimal gameplay snapshot needed for interaction and render highlighting

## Owned Data

### `SelectionState`

- `hovered_block`
- `hovered_face`
- `hit_point`
- `interaction_preview_blocks`
- `build_preview_block`

## Inputs

- `EcsInputSnapshot.cursor_screen_pos`
- `EcsInputSnapshot.focused`
- `EcsInputSnapshot.active`
- the current `CameraState`
- the local player `Transform` and `PlayerInventory`
- viewport width and height
- `WorldCore::raycast_blocks(...)`

## Outputs

- hovered block
- hovered face
- hit point
- interaction preview blocks
- build preview block
- a selection snapshot that `app::bridge` can convert into render highlights / previews

## State Rules

- Selection updates after app/world state has been refreshed for the current frame.
- The selection ray uses the same smoothed follow target and current zoom size that rendering uses.
- During the current turn-transition implementation, selection still uses the snapped gameplay quarter-view basis while rendering may briefly ease toward that basis.
- The current gameplay slice uses a weak perspective ray that starts at the camera eye and passes through a cursor-selected point on the quarter-view focus plane.
- if the inventory is open, selection is cleared so world interaction previews do not compete with the inventory UI
- interaction mode emits preview blocks when the hit point is inside the current tool reach; if no tool is selected, it falls back to a default single-block preview at player reach
- build mode emits a placement preview when the selected build quickslot contains a block, the adjacent cell is empty, and the preview is inside build reach
- If focus, activity, viewport, or cursor validity checks fail, selection is cleared.
- If raycast misses, selection is cleared.

## Invariants

- selection owns hover state only; it does not edit the world
- selection does not own world raycast algorithms and uses world queries instead
- renderers do not read `SelectionState` directly; `app::bridge` converts it into render-ready instances
- selection and rendering must stay aligned on follow target and zoom in the same frame, even if render-only turn easing is active
- preview generation is gameplay-owned and tool/mode-aware; the renderer only receives draw-ready cubes/slabs
- interaction preview blocks are also the target set consumed by prototype tool damage when a primary action is accepted

## Non-Responsibilities

- raw input capture
- world mutation
- renderer highlight shading
- hover dwell-time policy

## Related Modules

- `camera.rs`
- `runtime.rs`
- `world`
- `app/bridge.rs`
- `tool_interaction.rs`

## Notes

- Zooming the camera now changes the focus-plane selection footprint automatically because selection reads the ECS-owned current zoom value.
- interaction preview is now both visual feedback and the prototype tool-damage target set; build preview is the placement target consumed by app-owned command application
- preview texture choice is still bridge-owned: ECS emits block coordinates and mode-specific shapes, then `app::bridge` maps build placement previews to actual selected-block textures/materials
