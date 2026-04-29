# hydrology/water_surface

## Role

- decide whether a carved column can actually hold the already-resolved water profile after terrain shaping
- turn water-profile anchors into optional `water_surface_height` values only when local support exists, without changing the profile level
- derive a carve-supported water surface from the branch profile, target core floor, and desired visible depth so carving and final fill share the same effective water line
- preserve the generator order where terrain carve/deposition happens first and water visibility/fill is accepted or rejected last

## Boundaries

- may reject visible water from channel floor, terrain headroom, profile anchor, and flow signals
- may lower the effective water surface below the branch profile when the target bed/depth relation cannot support the higher profile
- must return the supplied supported surface unchanged when water is visible
- does not carve terrain and does not choose block ids
- downstream voxelization still owns quantized water block placement

## Invariants

1. visible water must have enough headroom above terrain and enough depth above the channel floor
2. water should not be emitted on uncarved shoulders merely because a corridor envelope is nearby
3. output is optional and must stay finite when present
4. this pass must not raise water to clear local terrain; terrain can only make the column dry or accept the lower carve-supported surface
5. standing water remains flat at the accepted supported level, and flowing branch samples may stay level or descend downstream but must not rise locally
6. if the supported surface differs from the branch profile, downstream carve/bar/fill passes must all use the supported surface consistently
