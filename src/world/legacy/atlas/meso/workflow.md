# meso feature workflow

## Read First

1. `context.md`
2. `src/world/world.md`
3. `src/world/atlas/meso.md`
4. `src/world/generation/meso_apply.md`
5. `src/world/atlas/meso/features/<feature>/`

If the feature does not exist yet, create its doc first and treat that doc as the contract.

## Core Contract

- `src/world/atlas/meso.rs` owns shared guide selection, sampling, and dispatch.
- `src/world/atlas/meso/features/<feature>/` owns feature-specific runtime resolve.
- `meso_apply.rs` should **stay thin**: gating, compositing, corridor policy, relief accounting.
- Do not rebuild a different landform guess per column or per chunk.
- Resolve stable world-space feature objects from a larger-than-chunk ownership unit first, then sample those same objects from neighboring chunks.
- Keep broad support separate from true core height/depth so transition fill cannot fake a summit or trench.

## Standard Files

- `src/world/atlas/meso/features/<feature>/<feature>.md`
- `src/world/atlas/meso/features/<feature>/mod.rs`
- optional feature-local files such as `resolved.rs`
- small integration changes in `src/world/generation/meso_apply.rs`
- tuning through `src/bin/meso_preview.rs`
- full-terrain validation through `src/bin/chunk_preview.rs`

## Workflow

### 1. Lock the feature doc

- Define role, scale, hydrology interaction, and shape goals.
- Define what belongs to atlas guide emission versus runtime resolve.
- Define preview success criteria before touching code.

### 2. Emit broad atlas guides

- Use atlas / region / structure context to emit deterministic multi-chunk guides only.
- Do not finalize the exact playable shape here.

### 3. Build stable runtime ownership

- Introduce a feature-owned resolved window or cache.
- Ownership should usually be `MesoRegion`-based or another stable unit larger than a chunk.
- Neighboring chunks overlapping the same footprint must sample the same resolved objects.

### 4. Resolve world-space feature objects

- Convert broad guides into explicit resolved objects.
- Vary bounded parameters per object so silhouettes do not repeat too obviously.
- Keep support and core as separate signals.

### 5. Return a surface-oriented sample

- Prefer a feature-owned surface contract over raw additive noise.
- Typical fields:
  - `target_surface_y`
  - `blend_weight`
  - `relief_spend`
  - optional local masks such as core / shoulder / support

### 6. Tune in `meso_preview` first

- Run on a flat base with the target feature isolated.
- Add overlays only if needed.
- Judge footprint, contour quality, overlap behavior, and seam continuity here first.

### 7. Validate in `chunk_preview`

- Check that the feature still reads on the real prototype.
- Check that corridor gating does not erase it.
- Check for chunk seams and feature-internal seams.
- Only after this should you touch cross-feature compositing in `meso_apply.rs`.

## Minimum Tests

- deterministic guide / resolve output for the same seed
- stable resolved ownership across neighboring chunk contexts
- no seam for the same world-space point across chunk or meso-cell boundaries
- flat response when guides are absent
- one feature-specific shape invariant
- material visible uplift/depth survives compositing

If the full lib target is noisy or slow, run exact tests for the feature and the relevant `meso_apply` integration tests.

## Default Commands

```bash
cargo test --bin meso_preview --quiet
cargo run --bin meso_preview -- <seed> --center-x <cx> --center-z <cz> --radius <r> --feature <feature>
cargo run --bin chunk_preview -- <seed> --stage prototype --center-x <cx> --center-z <cz> --radius <r> --width 1600 --height 900 --quarter-turns 0
```

## Done Criteria

- docs match code
- `meso_preview` clearly shows the intended shape
- `chunk_preview` still reads correctly without new seams
- feature-local seam / compositing regressions are covered by tests
- remaining `meso_apply.rs` changes are orchestration changes, not geometry logic
