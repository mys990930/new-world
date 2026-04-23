# bridge_input

## Role

- convert app-owned platform snapshots into ECS-owned frame input resources

## Responsibilities

- read current window focus / lifecycle activity from `Platform`
- normalize raw key / mouse / wheel state into `EcsInputSnapshot`
- keep raw input mapping out of gameplay systems and renderer code

## Inputs

- `Platform::window_state()`
- `Platform::raw_input_state()`
- `Platform::lifecycle_state()`

## Outputs

- one frame-local `EcsInputSnapshot` inserted into `EcsRuntime`

## Boundary Rules

- input meaning still belongs to ECS after the snapshot is created
- this bridge maps raw controls only; it does not mutate gameplay state directly
- `Ctrl + wheel` stays zoom while plain wheel stays quickslot cycling

## Invariants

- `Q/E` normalization must keep the existing ECS quarter-turn sign convention
- focus and activity state must travel with the frame snapshot
- direct quickslot selection stays a frame-local discrete field rather than UI-owned state

## Related Modules

- `bridge.md`
- `input.md`
- `../ecs/input.md`
- `../platform/input.md`
