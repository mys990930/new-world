# hydrology/water_surface

## Role

- decide whether a carved column can actually hold visible standing water after terrain shaping
- turn water-profile anchors into optional `water_surface_height` values only when local support exists

## Boundaries

- may compute final visible water height from channel floor, terrain headroom, profile anchor, and flow signals
- does not carve terrain and does not choose block ids
- downstream voxelization still owns quantized water block placement

## Invariants

1. visible water must have enough headroom above terrain and enough depth above the channel floor
2. water should not be emitted on uncarved shoulders merely because a corridor envelope is nearby
3. output is optional and must stay finite when present
