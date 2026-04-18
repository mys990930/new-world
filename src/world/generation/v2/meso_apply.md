# meso_apply

## Role

- own the step that applies atlas-owned meso accents on top of the base prototype

## Responsibilities

- refine the prototype without replacing the archetype-owned broad landform
- consume canonical meso guide fields rather than chunk-local feature picks
- preserve the continuity contract from `continuity.md` by keeping shared borders identical between neighboring solve tiles
- spend prototype relief budget on readable secondary landform accents

## Non-Responsibilities

- redefining the primary biome or terrain-form identity
- final hydrology network solve
- material or block choice

## Continuity Contract

- meso should run on the same shared solve tile model used by prototype
- global guide placement should come from canonical world-space guide fields or other deterministic region-owned functions
- any residual local deformation that is not itself canonical must fade to zero inside the border-anchor band
- meso may bend, terrace, or cluster landforms, but it must not reopen a seam that prototype already closed

## Planned Solve Strategy

1. sample canonical meso guide fields over the shared continuity tile
2. derive archetype-allowed meso accents from prototype relief budget plus guide ownership
3. apply secondary deformation while preserving protected corridor floors, ridge shoulders, and anchor bands
4. crop the requested chunk from the shared meso-applied tile

## Current Types

- `MesoAppliedPrototype`

## Notes

- meso should refine the archetype-owned prototype, not replace it
