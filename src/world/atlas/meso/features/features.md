# features

## Role

- hold one submodule and one design note per concrete meso feature type
- keep terrain and ecology intent close to the code stub for later implementation

## Contract

- every concrete feature should eventually have:
  - `mod.rs`
  - `<feature>.md`
- the Rust module should expose a lightweight `MesoFeatureDef`
- the markdown file should hold:
  - visual intent
  - terrain effect
  - hydrology relationship
  - ecology hooks
  - implementation cautions

## Current Stub Set

- `hill_cluster`
- `shallow_basin`
- `escarpment_band`
- `upland_terrace`
- `ravine`
- `coastal_cliff_band`
- `dune_field`
- `crater`
