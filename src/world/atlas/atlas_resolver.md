# atlas_resolver

## Role

- Reads atlas fields and chooses dominant `thermal`, `moisture`, `form`, `overlay`, and `biome preview` classes.

## Responsibilities

- Select dominant categorical classes from the weighted atlas field outputs.
- Produce a debug-facing biome preview that is easy to inspect while tuning atlas generation.
- Keep preview classification consistent with field-generation ownership and boundaries.

## Notes

- Resolver now honors the land/ocean split decided in `atlas_fields`.
- Ocean preview comes from the field overlay and land mask, not from a second raw `landness` cutoff.
- Overlay-driven categories such as `ocean`, `coast`, `wetland`, `riverine`, and `alpine` still have priority when their strength is high enough.

## Non-Goals

- It is not the authoritative final biome table for chunk realization.
- It does not assign materials, vegetation placement, or block-level realization rules.
- It should stay lightweight enough to support fast atlas iteration and debug image generation.
