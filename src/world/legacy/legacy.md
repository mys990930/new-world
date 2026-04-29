# legacy

## Role

- preserve the previous `world` module implementation during the graph-first world-generation redesign
- keep the current app, jobs, storage, meshing, tooling, and tests compiling while new contracts are introduced

## Responsibilities

- retain the old atlas-cell and chunk-generation implementation unchanged except for module visibility needed by the compatibility bridge
- expose the old public API through `src/world/mod.rs` until graph-first replacements are implemented
- keep old module documents available as historical implementation notes

## Non-Responsibilities

- defining the new graph-first architecture
- accepting new feature work unless the change is required to keep the legacy bridge compiling

## Migration Rule

- new design and implementation work belongs in `src/world/*.rs` next to the new documents
- code should move out of `legacy` only when its contract has been rewritten for the Voronoi graph pipeline
