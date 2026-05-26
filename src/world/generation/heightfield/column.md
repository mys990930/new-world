# heightfield/column

`column.rs` converts one `MacroFieldSample` into one `HeightfieldColumn`.

It owns per-column terrain-kind selection, normalized-to-block height resolve, contour/snap policy
application, and initial ocean/lake water hint assembly. Reusable ocean/lake helpers live in
`water.rs`; the tile-level river reset guard lives in `river.rs`.

Current river reset contract:

- River/estuary hints on `MacroFieldSample` are ignored for terrain carve and water creation.
- `RiverCore` / `RiverBed` terrain kinds are not emitted by column conversion.
- River diagnostic fields on `HeightfieldColumn` are written as neutral values.
- Perlin placement and contour gap selection do not special-case river hints.

The regression suite for this module lives in `column_tests.rs` and is included as a module-local
`#[cfg(test)]` child of `column.rs`.
