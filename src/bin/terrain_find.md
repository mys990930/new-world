# terrain_find

## Role

- find chunk candidates that match a requested launch archetype and optional meso preferences
- print ready-to-run `chunk_preview --stage prototype` commands for the strongest matches
- optionally render the best candidate immediately by shelling out to `chunk_preview`

## Inputs

- `seed`
- optional atlas search center via `--origin-cell-x <i32>` and `--origin-cell-z <i32>`
- optional atlas search radius via `--search-radius-cells <i32>`
- optional search density via `--chunk-step <u32>`
- optional repeated `--archetype <key>` launch-archetype filters
- optional repeated `--meso <key>` meso preference filters
- optional result count via `--top <usize>`
- optional preview selection via `--preview-rank <usize>`
- optional preview controls via:
  - `--preview-radius <i32>`
  - `--preview-width <u32>`
  - `--preview-height <u32>`
  - `--preview-quarter-turns <u8>`
  - `--preview-output <path>`
- optional `--render-preview` to execute `chunk_preview` automatically
- catalog helpers:
  - `--list-archetypes`
  - `--list-meso`

## Outputs

- stdout summary of the search area
- ranked chunk candidates
- per-candidate region identity and meso summary
- a ready-to-run prototype preview command for each candidate
- optional rendered PNG when `--render-preview` is passed

## Search Model

1. Generate atlas fields and structure for the requested atlas window.
2. Resolve the public launch-fallback region classes for that same window.
3. Generate atlas-owned meso guides for that window.
4. Sample region and meso state at each searched chunk center.
5. Filter by launch archetype when requested.
6. Score candidates by:
   - requested meso keys allowed by the current archetype
   - runtime-backed meso guide strength for supported Wave 1 keys
   - general meso interestingness and relief bonus for tie-breaking
7. Print `chunk_preview --stage prototype` commands for the best-ranked chunks.

## Archetype Notes

- exact archetype search currently supports only the launch archetype set
- this is intentional because the public `resolve_region_classes(...)` surface applies launch fallback before callers sample it
- use `--list-archetypes` to inspect the exact searchable keys

## Meso Notes

- `terrain_find` distinguishes two meso modes:
  - `runtime-backed`
    - the key has a direct current guide channel and contributes actual sampled guide strength
  - `planned-only`
    - the key exists in the catalog, but current guide generation does not emit it directly yet
    - in that case `terrain_find` still uses the archetype's `allowed_meso_keys` as a preference signal
- current runtime-backed meso keys are:
  - `hill_cluster`
  - `shallow_basin`
  - `escarpment_band`
  - `upland_terrace`
- many other meso keys remain cataloged and searchable as planned-only preferences:
  - for example `ravine`, `dune_field`, `crater`, `coastal_cliff_band`

## Preview Integration

- preview commands always target `chunk_preview --stage prototype`
- that path is chosen on purpose because prototype preview is the currently working terrain-visualization path in this repository
- `--render-preview` shells out to:

```bash
cargo run --bin chunk_preview -- <seed> --stage prototype ...
```

## Examples

List searchable launch archetypes:

```bash
cargo run --bin terrain_find -- --list-archetypes
```

List meso keys and whether they are runtime-backed:

```bash
cargo run --bin terrain_find -- --list-meso
```

Find temperate hills and print the best preview commands:

```bash
cargo run --bin terrain_find -- 42 --archetype temperate_hills --search-radius-cells 32 --chunk-step 2 --top 5
```

Find chunks that currently carry strong runtime `hill_cluster` guide weight:

```bash
cargo run --bin terrain_find -- 42 --meso hill_cluster --top 5
```

Find launch archetype terrain that allows the planned-only `ravine` meso key:

```bash
cargo run --bin terrain_find -- 42 --meso ravine --top 5
```

Find `temperate_hills`, then immediately render the best prototype preview:

```bash
cargo run --bin terrain_find -- 42 --archetype temperate_hills --search-radius-cells 32 --chunk-step 2 --top 1 --render-preview --preview-output target/terrain-find/temperate_hills.png
```

Search a larger atlas area more coarsely:

```bash
cargo run --bin terrain_find -- 42 --search-radius-cells 24 --chunk-step 2 --meso hill_cluster --top 10
```
