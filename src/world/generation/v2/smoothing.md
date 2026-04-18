# smoothing

## Role

- own post-meso smoothing and local refinement while preserving corridor and ridge intent

## Responsibilities

- reduce local harshness after prototype and meso deformation
- derive local slope and concavity signals for later hydrology and voxelization
- preserve corridor floors, ridge pressure, and tile-edge continuity while smoothing

## Non-Responsibilities

- inventing new broad landform identity
- moving anchored borders away from their shared edge truth
- solving connected water ownership by itself

## Continuity Contract

- smoothing should run as a constrained pass over the shared continuity tile
- border-anchor samples and other protected stage constraints should remain fixed or strongly clamped
- smoothing may redistribute interior noise, but it must not reintroduce shared-edge disagreement after prototype and meso have already aligned

## Planned Solve Strategy

1. consume the meso-applied shared tile plus the stage anchor set
2. mark hard or high-weight constraints such as anchor borders, corridor floors, basin outlets, and protected ridge lines
3. run constrained smoothing or relaxation across the tile interior
4. derive local refinement signals from the smoothed result and crop the requested chunk

## Current Types

- `SmoothedPrototype`

## Notes

- this stage should eventually derive local slope and concavity from the smoothed prototype
