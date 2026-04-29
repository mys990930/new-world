# field

## Role

- define continuous samples derived from the Voronoi graph before biome, heightfield, and material resolution
- keep visible terrain from sampling hard polygon ids directly

## Responsibilities

- represent blended graph influences around a world-space column
- represent continuous temperature, hydration, elevation, continentality, ruggedness, oceanness, and mountainness fields
- provide small normalization helpers for influence weights

## Non-Responsibilities

- building the Voronoi graph
- selecting final biome ids
- solving river flow
- filling blocks

## Invariants

1. hard polygon ownership may exist for cache/query identity, but visible terrain must sample blended fields.
2. influence weights are normalized before downstream terrain synthesis consumes them.
3. graph boundary distance is diagnostic and shaping input only; it must be warped and blended before it can affect visible output.

## Current Status

- this module is a scaffolded sampling contract
- future work should add graph-neighborhood sampling, spline-warped boundary distance fields, and cached field patches
