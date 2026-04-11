# atlas_fields

## Role

- Compute raw atlas fields and derived macro-environment factors.

## Responsibilities

- land / ocean mask
- continent id / continent core factor
- macro elevation / slope / ruggedness / ridge / mountain mass
- basin / river / lake / river distance
- temperature / humidity / inlandness
- aridity / wetness / polar / alpine / ecotone factor
- climate / moisture / form / overlay / cover weights

## Processing Order

1. compute landness and continent structure
2. compute ocean/coast distance and continent metadata
3. compute elevation, ridge, and mountain structure
4. compute drainage, river, and lake fields
5. compute temperature, humidity, and inlandness
6. compute derived factors and axis weights

## Invariants

1. Scalar fields must stay within documented normalized ranges.
2. Hydrology must remain coherent with land/ocean structure.
3. Distance-style fields must not collapse to zero just because a small sampled area lacks a local source cell.
4. Default tuning values live in `tuning.rs`, and field generation composes them rather than hard-coding alternate defaults elsewhere.
5. Reference seed scans used during tuning should still contain some non-trivial mountain signal so generation can realize more than rolling uplands.
