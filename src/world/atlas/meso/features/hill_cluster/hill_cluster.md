# hill_cluster

## Stage

- `launch`

## Identity

- placement family: `InteriorLandform`
- hydrology coupling: `AvoidPrimaryCorridor`

## Summary

- Deterministic multi-peak hill groups for an inland multi-chunk terrain accent.

## Terrain Traits

- Biases inland terrain prototypes without taking over the owning region identity.
- Runtime emission should read as several nearby hilltops under one shared cluster envelope instead of one low broad swell.
- At the current `0.5m` block scale, launch tuning should be strong enough that the resulting prototype deformation reads as actual hill country rather than sub-meter noise.
- Should avoid displacing major river corridors and instead sit beside or above them.

## Ecology Notes

- Later ecology can use this feature to break uniform cover into readable local habitat patches.
- This candidate is in the near-term planning set, so archetype allowances and deformation operators should be locked first.

## Follow-up

- lock final allowed-archetype coverage, deformation operator, and material/ecology hooks before implementation becomes authoritative
