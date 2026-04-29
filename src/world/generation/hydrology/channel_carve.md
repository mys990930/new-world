# hydrology/channel_carve

## Role

- compute the active-channel incision layers that turn smoothed terrain into a carved trough
- align the carved trough and low floodplain shoulder to the carve-supported water surface while preserving the later visible-water fill pass
- keep core, bank, floodplain, and outer cut deltas explicit so later passes can modify them without rediscovering river geometry

## Boundaries

- owns incision depth layering and the base carved terrain height
- consumes the branch/profile-derived supported water surface, target core floor, and reach-carve style resolved by the orchestration pass
- consumes bench lift adjustments supplied by floodplain/bar passes
- does not choose visible water, gravel material, or final hydrology mode
- does not fill water; `water_surface` decides visible water only after this carve has shaped terrain

## Invariants

1. channel carve may lower terrain but must not raise terrain above the smoothed source
2. nested cut layers must remain continuous and deterministic in world space
3. the core carve should match the target water-depth relation most strongly inside the active channel, while broader floodplain lowering should fade toward profile freeboard outside the wetted ribbon
4. shallow upper/source reaches should keep higher, narrower beds; lower broad reaches should lower and widen the floodplain/core more aggressively
5. later depositional passes should alter the base carve through explicit inputs, not hidden side effects
