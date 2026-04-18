# continuity

## Role

- define the shared continuity contract for V2 terrain realization stages after corridor solve
- turn seam continuity from a best-effort quality goal into an explicit solve boundary contract
- specify how canonical world-space fields, shared solve tiles, and border anchors fit together

## Goal

- any later V2 stage that can affect exposed terrain height should consume the same continuity contract
- the long-term target is that, after final exposed-surface quantization, no shared chunk edge differs by more than one block in height
- shared tile borders should agree exactly in continuous space before voxelization, so the remaining one-block budget belongs only to final integer surface expression

## Responsibilities

- define canonical world-space fields that are independent of chunk request order
- define the shared solve-tile footprint and halo contract that later stages should crop from
- define border-anchor ownership so neighboring tiles reuse the same edge truth
- define how stage-local residual detail may exist without reopening chunk or tile seams
- define cache and reuse expectations so canonical solves do not imply whole-world recomputation on every chunk request

## Non-Responsibilities

- choosing final block materials
- defining archetype catalogs
- deciding renderer-side contour or preview presentation
- replacing the corridor, prototype, meso, smoothing, or hydrology stage documents

## Continuity Contract

### 1. Canonical World-Space Fields

Every stage that affects terrain continuity should derive its large-scale intent from canonical world-space fields rather than from chunk-local candidate lists.

Examples:

- continuous atlas scalar samples
- branch-aligned corridor influence fields keyed by stable river identity
- atlas-owned meso guide fields sampled from deterministic region/meso ownership
- hydrology boundary fields such as downstream water-surface tendency or outlet floor tendency
- optional low-frequency world-space relief bases that are independent of chunk order

These fields must be functions of stable world coordinates plus seed/versioned data, not of the order in which chunks happen to be requested.

### 2. Shared Solve Tiles

Later realization stages should solve on a shared tile footprint rather than per chunk in total isolation.

Launch design target:

- core tile: `8 x 8` chunks
- halo: `2` chunks on each side
- solve footprint: `12 x 12` chunks

The exact launch constants may still change, but the contract should remain:

- a stage solves a larger haloed tile once
- requested chunks crop their interior from the shared core
- chunk seams inside the same core are therefore interior samples of one solve, not joins between two separately solved chunks

Current prototype implementation note:

- the current code uses a smaller `4 x 4` continuity tile with no extra halo yet
- prototype still returns one chunk at a time, but it now samples a shared tile corridor context, reuses cached tile/chunk continuity state, and treats tile-border anchors as canonical edge truth

### 3. Border Anchors

Shared tile borders should not depend on whichever neighboring tile solved first.

A border anchor is a canonical boundary truth for a stage-aligned edge sample in world space. Neighboring tiles that share a border must query the same anchor samples.

An anchor sample may carry:

- target terrain height
- optional slope or normal tendency
- optional local relief cap
- optional water-surface or channel-floor tendency for later hydrology-aware stages

At minimum, prototype through hydrology need stable height anchors.

### 4. Residual Detail Rule

Local stage detail is allowed only if it does not reopen seams.

That means each stage should conceptually split into:

- a canonical global basis
- a local residual term

The residual term must either:

- be defined by the same canonical world-space function everywhere, or
- fade to zero across the border-anchor band so the shared border remains identical between neighboring tiles

## Planned Data Shapes

- `GenerationTileCoord`
- `GenerationTileBounds`
- `GenerationTileHalo`
- `BorderAnchorSample`
- `BorderAnchorBand`
- `StageAnchorSet`

These names are design placeholders, not yet locked implementation types.

Current prototype code now has the first concrete subset of these ideas:

- `GenerationTileCoord`
- `GenerationTileBounds`
- `BorderAnchorPoint`
- per-run and shared caches for prototype chunk state and tile corridor context

## Stage Expectations

### Prototype

- solve broad terrain on the shared tile footprint
- derive its primary frame from canonical world-space fields plus corridor branch fields
- obey border anchors exactly on the tile edge and blend the first few interior columns back toward the interior solve
- current code also applies a narrow chunk-edge continuity blend inside the tile while prototype is still analytically sampled per requested chunk instead of materializing one persistent tile grid

### Meso Apply

- place accents from canonical meso guide fields or other world-space deterministic functions
- any residual deformation that is not itself canonical must fade out inside the anchor band
- should never reintroduce a step along a shared chunk or tile edge

### Smoothing

- run as a constrained smoothing or relaxation pass over the shared tile
- anchors and protected corridor/ridge constraints remain fixed or strongly clamped during smoothing
- smoothing exists to reduce local harshness, not to erase anchor agreement

### Hydrology

- solve connected waterlines, outlet floors, and floodable surfaces from canonical drainage ownership
- use border anchors so neighboring tiles agree on water-surface and channel-floor tendencies before voxelization
- regional or tile caches are expected here because this is the most expensive globalized stage

### Voxelization

- convert the anchored continuous surface into block columns without reintroducing edge disagreement
- exposed-surface quantization should respect the continuity target instead of independently rounding each chunk edge in isolation

## Cache and Cost Model

Continuity should not imply whole-world recomputation on every chunk request.

Expected strategy:

- canonical world-space fields are sampled directly or cached by atlas/region ownership
- heavy stage solves are cached per shared solve tile
- hydrology may additionally use larger drainage-region caches behind the tile-facing interface

This keeps seam behavior deterministic while containing the cost of globalized stages.

## Validation Target

- shared chunk-edge regression tests should exist for prototype, meso, smoothing, hydrology, and final exposed surface
- tile-border tests should confirm that two neighboring tiles emit identical border-anchor samples
- the final gameplay-facing target is `shared exposed-surface delta <= 1 block` across the world, regardless of chunk or tile generation order

## Implementation Order

1. canonicalize corridor-driven prototype inputs around stable branch fields
2. introduce shared prototype tile solve plus border anchors
3. move meso deformation onto the same continuity contract
4. implement anchor-constrained smoothing
5. implement hydrology over canonical drainage ownership with cached tile/regional solves
6. add end-to-end seam regression suites through final exposed-surface output
