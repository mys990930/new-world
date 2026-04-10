# profiles

## Role

- Hold the profile-specific surface shaping functions used by generation.

## Responsibilities

- dispatch from `TerrainProfile` to the matching surface function
- keep ocean, coast, inland, and ridge shaping logic isolated

## Current Modules

- `ocean.md`
- `coast.md`
- `plain.md`
- `upland.md`
- `ridge.md`

## Notes

- Each profile module turns the same sampled atlas signals into a different height curve and relief pattern.
- This keeps future material, vegetation, and structure generation aligned around the same profile boundary.
