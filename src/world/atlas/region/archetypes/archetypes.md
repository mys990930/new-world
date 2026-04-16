# archetypes

## Role

- hold one submodule and one design note per concrete `RegionArchetype`
- keep regional identity, ecology notes, seasonal behavior, and meso allowances attached to the type itself

## Contract

- every concrete archetype should eventually have:
  - `mod.rs`
  - `<type>.md`
- the Rust module should expose a lightweight `RegionArchetypeDef`
- the markdown file should hold planning content:
  - identity
  - terrain traits
  - hydrology expectations
  - ecology notes
  - seasonal notes
  - allowed meso

## Current Stub Set

- `temperate_plain`
- `temperate_plateau`
- `temperate_hills`
- `tropical_rainforest_lowland`
- `tropical_rainforest_hills`
- `desert_plain`
- `cold_wet_lowland`
- `glaciated_alpine`
- `coastal_cliffland`
