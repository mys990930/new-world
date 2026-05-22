# player_visual

## Role

- Define the render-facing local-player visual contract owned by ECS.
- Keep the collision body and gameplay locomotion separate from the voxel character rig.
- Provide a stable bridge target for app-owned conversion into renderer DTOs.

## Owned Data

### Render-Facing State
- `VoxelPlayerVisualState`
- `VoxelPlayerAnimationState`
- `VoxelPlayerFacingOctant`

### Rig / Pose Description
- `VoxelPlayerPart`
- `VoxelPlayerPartPose`
- `default_voxel_player_part_poses()`

## Inputs

- local player `Transform`
- local player `Velocity`
- local player `PlayerPhysicsState`
- `LocalPlayerEntity`
- frame/fixed time in later animation work

## Outputs

- render-facing player root translation
- render-facing 8-octant direction
- coarse animation state such as idle, walk, sprint, or airborne
- default voxel body-part layout that app bridge can expand into dynamic cubes

## State Transition Rules

- gameplay collision remains driven by `PlayerBody`; visual parts must fit inside or intentionally decorate around that body
- visual state is derived from ECS-owned player movement and physics state, not renderer-side velocity inference
- facing uses horizontal velocity when the player is moving and preserves a deterministic default when stationary
- initial animation states are intentionally coarse:
  - `Idle` when grounded and nearly stationary
  - `Walk` when grounded and moving below sprint-scale speed
  - `Sprint` when grounded and moving at sprint-scale speed
  - `Airborne` when not grounded
- the default rig is code-authored data, not an external atlas or sprite dependency
- part poses are local to the player root; app bridge is responsible for composing root, facing, animation offset, and renderer DTOs

## Invariants

- ECS owns gameplay-facing visual state selection
- renderer must not infer player intent, movement state, or facing from raw transform deltas
- app bridge may approximate the rig with axis-aligned cube instances until renderer supports rotated dynamic parts
- future animation must remain deterministic from explicit ECS state plus time, so multiplayer/server replay can reproduce visual state selection
- visual scaffolding must not change `PlayerBody`, collision, spawn placement, selection, or camera follow semantics

## Non-Responsibilities

- GPU resource creation
- draw ordering
- texture atlas generation
- world collision
- gameplay input interpretation

## Related Modules

- `player.rs`
- `runtime.rs`
- `../app/bridge.md`
- `../renderer/renderer.md`

## Implementation Plan

1. Keep `PlayerBody` as the physical source of truth and add a separate `VoxelPlayerVisualState` snapshot for app bridge reads.
2. Start with a code-authored six-part rig: head, torso, left/right arms, and left/right legs.
3. Bridge the rig to multiple dynamic cube instances, replacing the current single white player cube while retaining the existing ground shadow.
4. Add procedural animation offsets in the bridge or a small ECS helper:
   - idle bob
   - mirrored arm/leg swing for walk and sprint
   - airborne tuck / raised-arm pose
5. If axis-aligned limb approximation is not expressive enough, extend renderer DTOs with an oriented dynamic cube instance while keeping gameplay-facing pose selection in ECS.
6. Add deterministic tests for visual-state classification, facing octant selection, and bridge-emitted part count before replacing the dummy cube path.

## Notes

- The first integrated version should favor readable motion and low risk over a full skin system.
- Hand-authored colors and part proportions are acceptable for the first runtime player because they avoid asset loading and atlas ownership questions.
- Later equipment, held items, and skins should extend the rig data rather than changing collision dimensions.
