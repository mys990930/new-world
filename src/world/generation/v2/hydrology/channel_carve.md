# hydrology/channel_carve

## Role

- compute the active-channel incision layers that turn smoothed terrain into a carved trough
- keep core, bank, floodplain, and outer cut deltas explicit so later passes can modify them without rediscovering river geometry

## Boundaries

- owns incision depth layering and the base carved terrain height
- consumes bench lift adjustments supplied by floodplain/bar passes
- does not choose visible water, gravel material, or final hydrology mode

## Invariants

1. channel carve may lower terrain but must not raise terrain above the smoothed source
2. nested cut layers must remain continuous and deterministic in world space
3. later depositional passes should alter the base carve through explicit inputs, not hidden side effects
