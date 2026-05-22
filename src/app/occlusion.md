# occlusion

## Role

- Build render-facing terrain fade targets for blocks that visually cover the local player.

## Responsibilities

- Sample a small set of local-player body points.
- Walk from those samples toward the current render camera eye.
- Collect loaded solid world blocks between the camera and player.
- Provide a player-centered fade focus and radius pair once any covering block is found.
- Keep the collected block list small and deterministic for renderer uniform upload.

## Non-Responsibilities

- Mutating world block data.
- Changing chunk mesh generation or upload.
- Owning renderer shader behavior.
- Interpreting gameplay commands.

## Inputs

- `WorldCore`
- local-player `Transform`
- local-player `PlayerBody`
- current `RenderCameraState`

## Outputs

- `OccludingBlockSet` containing `WorldBlockCoord` values plus player-centered vignette parameters.

## Boundary Rules

- The app owns this presentation policy because it coordinates ECS player/camera state with world reads before rendering.
- The world remains the source of truth for block solidity; this module only queries loaded blocks.
- The renderer receives render-ready block coordinates and decides how to draw the fade.

## Invariants

- Occlusion collection must be bounded per frame.
- Missing/unloaded chunks are treated as empty for presentation; they must not trigger synchronous loading.
- Player occlusion fade must not request remeshes or renderer mesh uploads.
- Exact occluding blocks only enable and bound the effect; the visual mask is a soft player-centered vignette so the fade reads as a visibility aid rather than individual block toggles.
