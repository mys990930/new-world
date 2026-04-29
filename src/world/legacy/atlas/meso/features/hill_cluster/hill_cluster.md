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
- Atlas-owned guide emission should describe broad multi-chunk hill propensity with several strong guide cells across a few chunks rather than a final point-like object list.
- Chunk-side realization should resolve sparse independent hills from dominant local peaks, with each resolved hill owning a primary irregular blob, an optional secondary blob, and a low-amplitude support shoulder.
- The runtime helper should own the hill surface resolve itself instead of returning only a generic additive `delta_y`.
- Chunk-side realization should not reconstruct a different hill guess per sampled column; it should sample a shared set of world-space hill objects whose ownership is stable at the meso-region layer rather than re-owned per chunk.
- A resolved hill cluster should produce one feature-owned target surface that rises several blocks above the local prototype baseline, with broad shoulders and soft falloff blended back into the surrounding plain.
- At launch tuning, strong hill-cluster lobes should be able to exceed roughly `+8` blocks relative to the nearby prototype where guide support and archetype allowance justify it.
- Dense neighboring guide cells should be filtered into a smaller set of dominant hill seeds per meso-region so one window does not degenerate into many tiny overlapping bumps.
- Individual hill lobes should be visibly irregular and asymmetric rather than reading as clean circular blobs.
- Closed contours should undulate and wobble enough that the hill read stays organic rather than tracing obvious circles or ellipses.
- Launch tuning should prefer fewer, broader, taller independent hills over many small peaks packed into the same area.
- At the current `0.5m` block scale, launch tuning should produce several-meter prototype relief so the feature reads as real hill country rather than sub-meter noise.
- Launch tuning should also favor a clearly broader few-chunk footprint so each hill has room to read before the next saddle begins, but should avoid support staying alive across so many chunks that no individual hill reads clearly in top-down preview.
- Launch tuning should avoid overly extreme major-axis stretch so broad hill groups still read as clustered hills, not as a single ridge-like strip.
- The resolved shape should keep a broader shoulder support zone than the inner hill cores so plains can ease upward naturally instead of stepping into a hard wall.
- Current launch tuning target is roughly `3..6` chunks for the visually obvious hill core and roughly `6..9` chunks for the full shoulder/support footprint, with owner-region sparsening chosen to avoid one resolved hill set swallowing large parts of a preview window.
- Current launch tuning should keep ownership pruning conservative enough that nearby strong sources do not immediately collapse into one oversized bright mass across a whole preview edge.
- Nearby hills may overlap through low shared support, but should not form circular Venn-diagram plan-view saddles or a synthetic shared summit.
- Different hills should keep different summit ceilings within a tuned min/max band so nearby peaks do not all stall at the same y level; resolved blob caps should vary from hill to hill.
- Summit profiles should stay rounded and sigmoid-like from plain -> side -> top, with support fading back out before the apex so the summit does not read as a flat cap above a mostly constant side slope.
- Broad shoulder support should help hills ease back into plains, but should not by itself saturate to the same near-peak uplift as the summit core in top-down preview.
- Support fill should remain a foothill / transition signal, not a hidden substitute for blob-core height that can create a broad synthetic summit where no direct hill core exists.
- Current launch tuning should bias strongly toward wider x/z footprint rather than higher caps so hills read gentler at the game's `0.5m` block scale.
- Irregular contour wobble and notch carving should stay present, but be damped enough that most hills still read as rounded landforms rather than sharp star-shaped blobs.
- Corridor-adjacent hills may be damped, but the feature should still keep a materially visible uplift where the resolved hill mass survives beside the corridor.
- The chunk-side helper must scan a wide enough neighboring guide neighborhood that the same broad hill mass does not disappear or clip when the sample crosses a chunk or meso-cell boundary.
- Neighboring chunks that overlap the same hill footprint should resolve the same world-space hill blobs from the same meso-region-owned hill set, not just similar guide samples.
- When a hill has no nearby dominant neighbor to suggest a heading, fallback orientation must stay source-stable across neighboring samples instead of re-rolling per chunk or per meso cell.
- Should avoid displacing major river corridors and instead sit beside or above them.

## Ecology Notes

- Later ecology can use this feature to break uniform cover into readable local habitat patches.
- This candidate is in the near-term planning set, so archetype allowances and deformation operators should be locked first.

## Follow-up

- lock final allowed-archetype coverage, feature-owned surface resolver behavior, and material/ecology hooks before implementation becomes authoritative
