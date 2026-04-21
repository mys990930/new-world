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
- Atlas-owned guide emission should describe one shared inland cluster envelope with several strong guide cells across a few chunks rather than a point-like object list.
- Chunk-side realization should resolve that guide into coherent asymmetric macro-lobe chains with visible saddles between nearby hills, so the player reads hill country rather than circles, tiny local hilllets, or one smeared swell.
- The runtime helper should own the hill surface resolve itself instead of returning only a generic additive `delta_y`.
- Chunk-side realization should not reconstruct a different hill guess per sampled column; it should first resolve a shared set of world-space hill objects for the current chunk window and then sample those objects.
- A resolved hill cluster should produce one feature-owned target surface that rises several blocks above the local prototype baseline, with broad shoulders and soft falloff blended back into the surrounding plain.
- At launch tuning, strong hill-cluster lobes should be able to exceed roughly `+8` blocks relative to the nearby prototype where guide support and archetype allowance justify it.
- Dense neighboring guide cells should be pruned into a smaller set of dominant hill masses so one cluster does not degenerate into many tiny overlapping bumps.
- Individual hill lobes should be visibly irregular and asymmetric rather than reading as clean circular blobs.
- Launch tuning should prefer fewer, broader, taller hills over many small peaks packed into the same area.
- At the current `0.5m` block scale, launch tuning should produce several-meter prototype relief so the feature reads as real hill country rather than sub-meter noise.
- Launch tuning should also favor a clearly broader few-chunk footprint so each hill mass has room to read before the next saddle begins.
- Launch tuning should avoid overly extreme major-axis stretch so broad hill groups still read as clustered hills, not as a single ridge-like strip.
- The resolved shape should keep a broader shoulder envelope than the inner hill cores so plains can ease upward naturally instead of stepping into a hard wall.
- Corridor-adjacent hill clusters may be damped, but the feature should still keep a materially visible uplift where the resolved hill mass survives beside the corridor.
- The chunk-side helper must scan a wide enough neighboring guide neighborhood that the same broad hill mass does not disappear or clip when the sample crosses a chunk or meso-cell boundary.
- Neighboring chunks that overlap the same hill footprint should resolve the same world-space hill blobs, not just similar guide samples.
- When the resolved source layout is too compact to infer a clear shared axis, fallback orientation must stay source-stable across neighboring samples instead of re-rolling per chunk or per meso cell.
- Should avoid displacing major river corridors and instead sit beside or above them.

## Ecology Notes

- Later ecology can use this feature to break uniform cover into readable local habitat patches.
- This candidate is in the near-term planning set, so archetype allowances and deformation operators should be locked first.

## Follow-up

- lock final allowed-archetype coverage, feature-owned surface resolver behavior, and material/ecology hooks before implementation becomes authoritative
