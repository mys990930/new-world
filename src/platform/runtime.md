# runtime

## Role

- winit adapter
- window creation
- OS/winit event collection
- `PlatformEvent` normalization
- window/input/lifecycle reducer fan-out

## Responsibilities

- own the live event-loop and window bootstrap boundary
- normalize raw OS events into `PlatformEvent`
- dispatch normalized events into the window/input/lifecycle reducers
- expose the live `Arc<Window>` handle for app/renderer attachment

## Non-Responsibilities

- top-level game loop orchestration
- fixed timestep ownership
- ECS execution
- renderer draw scheduling
- gameplay input interpretation
- world mutation

## Owned Data

- `Arc<Window>`
- event-loop runtime context
- normalized event dispatch flow

## Process

1. Receive raw OS/winit events.
2. Normalize them into `PlatformEvent`.
3. Fan out zero or more normalized events from a single OS event when needed.
4. Dispatch the normalized events into the window/input/lifecycle reducers in order.
5. Leave higher layers with snapshot state they can safely read.

## Public Interface

```rust
PlatformRuntime::new() -> PlatformRuntime
PlatformRuntime::resumed(...)
PlatformRuntime::suspended(...)
PlatformRuntime::handle_window_event(...)
PlatformRuntime::request_redraw(&self)
PlatformRuntime::window_handle(&self) -> Option<Arc<Window>>
```

## Dependencies

- winit

## Invariants

- reducers do not depend directly on winit types
- runtime does not create gameplay state
- the live window remains available as `Arc<Window>` so app/renderer can attach safely

## Related Modules

- `mod.rs`
- `event.rs`
- `window.rs`
- `input.rs`
- `lifecycle.rs`

## Notes

- the current implementation fans out `PlatformEvent` values directly into the reducers without per-event console logging
- higher layers should log intentional summaries only when that helps debugging
