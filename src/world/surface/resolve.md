# resolve

## Role

- resolve one chunk-column surface plan from region ownership, smoothed terrain hints, hydrology output, and optional runtime season context
- hand voxelization quantized terrain / water tops plus chosen block keys without re-solving landform or water geometry

## Responsibilities

- map `RegionArchetype` to one launch-oriented `MaterialPolicyId`
- resolve region influence transitions into visual material choices without exposing atlas-cell rectangles
- resolve an optional `SeasonalBiomeStateId` when runtime season context is available
- derive `CoverPhase` and small `CoverOverrideRule` hooks such as `snowy_grass`
- choose per-column:
  - top block key
  - filler block key
  - core block key
  - optional water or ice block key
  - filler depth
- quantize terrain top and standing-water top for the later block write stage

## Non-Responsibilities

- atlas or region classification itself
- meso placement
- hydrology carve or connected-water solving
- final block writes into `ChunkData`
- world calendar progression

## Current Contract

- region ownership remains deterministic per sampled column for query/storage, but visible material ownership now comes from support-driven competition among the foreign domains present in the shared region influence neighborhood, plus deterministic displaced material boundaries
- generation-derived material support axes from `SmoothedColumn` are part of the visible material
  contract; hard atlas ownership alone must not force mud, scree, sand, grass, or rock when the
  local support field says otherwise
- hydrology is allowed to override sediment and wet-surface expression
- hydrology, broad material support, and transition strength should deform broad material boundaries before block keys are chosen
- local slope, concavity, and height bands must not change the normal cover block across an otherwise identical visible environment; terrain-local variation is reserved for hydrology-owned bars/beds and explicit coastal/cliff features
- broad exposed-rock, beach, shelf, and coastal-cliff material choices should stay coherent categorical areas; artifact suppression should move their edges or tie them to terrain/hydrology features rather than mottling their interiors
- hydrology-driven overrides should dominate only when the solved water or saturation signal is strong enough to justify a wet surface; weak floodplain context should still allow biome and transition breakup
- plain `Floodplain` / low-saturation channel margins without visible water should stay on biome-owned tops by default instead of automatically collapsing into mud
- hydrology-provided standing water must be quantized from the resolved level without re-solving or raising it; if the quantized level does not clear `terrain_top_y`, the column is dry
- coastal and marine contexts may promote sea-level standing water even when inland hydrology did not emit it
- the default `generate_chunk(...)` path currently uses no runtime season context, so seasonal state may stay `None`
- explicit runtime seasonal context remains the future integration point for `WorldCore` calendar ownership
- surface resolve consumes the shared atlas `sample_region_class_influences(...)` output instead of deriving its own atlas-grid fractional sampler
- surface resolve still samples hard owner separately for storage/query, while visible material policy is selected from the resolved material domain before final transition breakup
- visible material policy may be selected from the resolved material domain, not only from the
  visible owner's archetype, so local terrain-supported domain overrides are not discarded before
  block-stack selection

## Public Surface

```rust
resolve_material_policy_for_archetype(archetype: RegionArchetype) -> MaterialPolicyId

resolve_chunk_surface_plan(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationInputs,
    smoothed: &SmoothedPrototype,
    hydrology: &HydrologySolve,
) -> ChunkSurfacePlan

resolve_chunk_surface_plan_with_runtime(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationInputs,
    smoothed: &SmoothedPrototype,
    hydrology: &HydrologySolve,
    runtime: Option<&SurfaceRuntimeContext>,
) -> ChunkSurfacePlan
```

## Material Transition Contract

- surface resolve is the final shield against visible atlas-cell material rectangles.
- top/filler/core block choice should start from the domain-warped visible material owner, while preserving the hard sampled owner as `owner_archetype` for gameplay/query surfaces.
- transition selection must be deterministic from world position and seed-derived fields, not from chunk order.
- transition selection should behave like a displaced boundary: each column still resolves to one clear block stack, while the line where that stack changes is warped by broad hydrology/material support.
- a domain-warped visible owner that crosses the hard atlas owner must be supported by local terrain or hydrology evidence; a large smooth curve from warp alone is still an artifact.
- visible material influence sampling may be displaced by smoothed material support and strong
  hydrology. This displacement is a final boundary-placement tool, not a new owner for gameplay
  queries.
- resolver-side boundary displacement should stay weaker than region influence and material-domain
  support. It may soften a tie, but must not draw a second macro curve independent of the accepted
  environment boundary.
- after the first per-column plan pass, surface resolve may apply a deterministic visual boundary
  stepping pass. This pass can copy visual material fields from a directly adjacent dry neighbor
  when the two columns already share a material boundary, then grow that copied visual material for
  a few columns only through cardinal adjacency to the same source material. This gives the line a
  block-scale wobble without diagonal-only connections or scattered interior spots.
- the visual boundary stepping pass runs on a one-column halo around the target chunk and then
  crops back to the central chunk columns, so material steps that cross chunk edges are judged with
  the same world-position noise instead of being clipped by the local chunk border.
- halo cells may extend an existing interior boundary across a chunk edge, but they must not create
  a new edge-only dotted boundary when the target chunk has no cardinally adjacent internal
  material transition at that edge.
- material support may decide where the boundary falls, but it must not recolor every height,
  slope, or contour band inside a single environment. Once a column belongs to the same visible
  environment, its cover block should stay stable unless hydrology, season, coast, or a clear
  cliff/coastal feature owns the variation.
- compatible dry/temperate/steppe/savanna/plateau/alpine/wetland boundaries should form coherent
  categorical regions rather than intermediate block speckles or salt-and-pepper mixing.
- hydrology-driven materials such as silt, mud, wet sand, gravel bars, peat, water, and ice remain stronger than biome boundary displacement near channels, lakes, wetlands, coasts, and high-saturation floodplain cells.
- wetland and alpine policies use one stable default cover inside the accepted visible environment.
  Broad support moves the environment boundary; it does not swap mud/dirt or scree/thin-soil at
  every local support or height band.
- sandy beach and oceanic shelf outputs may switch to gravel or wet gravel only where terrain slope, concavity, or gravel-bar support justifies a connected feature.
- dry oceanic-shelf and coastal-cliff interiors may resolve to connected sand, gravel, or rock surfaces from sea-level-relative height, slope, and concavity; this is terrain-supported subdivision, not random mottling.
- true macro barriers may stay sharper, but their edge should still be terrain-following or hydrology-supported rather than an atlas grid line.
- surface resolve must not solve atlas artifacts by adding per-block visual noise over the middle of an otherwise uniform material province.
