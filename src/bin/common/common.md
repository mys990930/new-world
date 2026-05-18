# common

## Role

`src/bin/common` owns helper code shared by standalone binaries under `src/bin`.

These helpers are binary adapter utilities: preview drawing primitives, compass overlays, and thin
preview stage setup glue. They are not part of world generation source-of-truth.

## Responsibilities

- Shared RGB/RGBA preview drawing primitives such as pixel blending, simple line/ring/text helpers,
  and small panel/legend utilities.
- Shared topdown compass helpers for preview images.
- Thin opt-in graph-first preview setup helpers that call world-owned generation APIs in the
  documented stage order.

## Non-Responsibilities

- Terrain policy, hydrology policy, biome classification, macro field semantics, pixelize or
  heightfield rules.
- CLI parsing, output filenames, PNG metadata key names, channel meanings, or stage-specific color
  palettes.
- Runtime world storage mutation or renderer/GPU resource ownership.

## Invariants

1. Helpers under `src/bin/common` must not create new generation semantics.
2. Preview binaries keep ownership of their CLI, metadata, stdout, and visual interpretation.
3. Shared stage setup must remain a thin wrapper around public `world::generation` APIs.
4. If a preview needs specialized windowing, projection, or overlay policy, that logic stays local
   to the binary.
