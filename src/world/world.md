# world

## Role

- own source-of-truth world data and deterministic world generation contracts
- define the graph-first macro terrain model for the next generator
- keep chunk storage, block registry, meshing input, and saved-world compatibility inside the world boundary

The new generation direction replaces the old square atlas-cell-first terrain identity with a
Voronoi graph based macro layer. Chunks remain voxel storage/output windows only; they must not
own biome, hydrology, mountain, coast, or regional terrain identity.

## Responsibilities

- loaded chunk storage and mutation API ownership
- block id, block definition, texture tile, and material lookup ownership
- save/load byte codec ownership
- CPU-side meshing input and world-owned geometry meaning
- graph-first macro region ownership through Voronoi sites, corners, and edges
- continuous field ownership for temperature, hydration, elevation, continentality, ruggedness, oceanness, and mountainness
- graph hydrology ownership through selected Voronoi-edge drainage paths, watersheds, outlets, lakes, and sinks
- biome and surface policy resolution from blended continuous fields rather than hard polygon ids
- heightfield synthesis from base noise plus graph-derived gradients and hydrology constraints
- deterministic procedural generation result expression as `ChunkData`
- legacy runtime compatibility while graph-first modules are scaffolded

## Non-Responsibilities

- visible chunk selection
- gameplay command interpretation
- fixed tick scheduling
- async worker orchestration
- GPU buffer creation or draw calls
- platform input/window handling

## Owned Data

- legacy runtime data retained during migration:
  - `WorldMeta`
  - `ChunkCoord`, `LocalBlockCoord`, `WorldBlockCoord`
  - `BlockId`, `BlockFace`
  - `BlockDef`, `BlockRegistry`
  - `ChunkData`, `ChunkSnapshot`
  - `WorldCore`
  - `WorldEdit`, `EditResult`
  - storage, topdown, tree, surface, and meshing data shapes
- graph-first scaffold data:
  - `GraphRegionCoord`, `GraphRegionArea`
  - `VoronoiSiteId`, `VoronoiCornerId`, `VoronoiEdgeId`
  - `VoronoiSite`, `VoronoiCorner`, `VoronoiEdge`, `VoronoiGraphPatch`
  - `ContinuousFieldSample`, `GraphInfluence`, `VoronoiBlendSample`
  - `WatershedId`, `GraphDrainageNode`, `GraphRiverSegment`, `GraphHydrologyGraph`
  - `GraphWorldGenerationConfig`, `GraphGenerationStage`
  - `ColumnSynthesisRequest`, `ColumnSynthesisSample`

## Public Interface

Current runtime compatibility remains available through re-exported legacy APIs:

```rust
WorldCore::new(meta: WorldMeta, block_registry: Arc<BlockRegistry>) -> WorldCore
generation::generate_chunk(coord: ChunkCoord, meta: &WorldMeta, registry: &BlockRegistry) -> ChunkData
build_chunk_mesh(snapshot: &ChunkSnapshot, registry: &BlockRegistry, neighbors: NeighborChunks) -> CpuMesh
storage::load_chunk(bytes: &[u8]) -> Result<ChunkData, StorageError>
storage::save_chunk(snapshot: &ChunkSnapshot) -> Result<Vec<u8>, StorageError>
```

New graph-first scaffold APIs:

```rust
graph_region_for_world_block(world_x: i32, world_z: i32, region_size_blocks: i32) -> GraphRegionCoord
GraphRegionArea::new(min: GraphRegionCoord, max: GraphRegionCoord) -> Option<GraphRegionArea>
VoronoiGraphPatch::site(id: VoronoiSiteId) -> Option<&VoronoiSite>

normalize_influences(influences: &mut [GraphInfluence])
ContinuousFieldSample::clamped(self) -> ContinuousFieldSample

GraphHydrologyGraph::segments_for_edge(edge: VoronoiEdgeId) -> impl Iterator<Item = &GraphRiverSegment>
graph_generation_stages() -> &'static [GraphGenerationStage]
```

## Graph-First Generation Model

### 1. Macro Voronoi Graph

- Generate deterministic Poisson/jittered sites per graph region with padding.
- Build site, corner, and edge graph patches from seed, generator version, and graph region ownership.
- Store graph regions as cache units only; never expose their rectangular boundaries as visible terrain.

### 2. Continuous Region Fields

- Assign site-level temperature, hydration, elevation bias, continentality, and ruggedness.
- Smooth or relax neighboring site values so adjacent polygons have plausible continuity.
- Sample columns through weighted graph influence, not through nearest-cell hard labels.

### 3. Land, Ocean, and Mountain Gradients

- Derive continent/ocean tendency from graph-scale basins, distance-to-coast candidates, and low-frequency noise.
- Derive mountainness from selected graph chains, ridge distance fields, and ruggedness.
- Blend those gradients with base fBm/OpenSimplex-style noise during heightfield synthesis.

### 4. Hydrology Graph

- Treat Voronoi edges and corners as candidate drainage structure.
- Select only some edges as rivers or divides based on elevation, watershed routing, rainfall/hydration, and outlet solving.
- Resolve local minima as lakes, sinks, or carved outlets before chunk voxelization.
- Realize selected river paths as spline/domain-warped corridors, not raw straight polygon edges.

### 5. Biome and Surface Resolve

- Resolve biome influence from continuous temperature, hydration, elevation, hydrology, and dominant region ownership.
- Use hard graph ownership only where gameplay/query stability needs a stable owner.
- Use blended fields and warped boundaries for visible material transitions.

### 6. Heightfield and Voxel Fill

- Synthesize the final column height from base noise, graph-derived gradients, mountain fields, water corridors, basin/lake flattening, and local detail.
- Convert the resolved column plan into `ChunkData` without allowing chunk boundaries to affect the result.

## Dependencies

- may use Rust standard library and project-local world types
- may use save format configuration

NOT:

- `app`
- `ecs`
- `renderer`
- `platform`

## Invariants

1. block and chunk mutations only happen through world-owned APIs.
2. chunk coordinates select output windows only; macro terrain identity comes from graph/hydrology/field ownership.
3. graph region rectangles, Voronoi polygon edges, and chunk boundaries are internal ownership/cache shapes, not visible terrain masks.
4. visible terrain samples blended continuous fields and spline/domain-warped boundaries instead of hard nearest-polygon labels.
5. Voronoi edges are hydrology candidates, not automatic rivers.
6. hydrology is solved before final heightfield commitment and voxel fill.
7. the same `(seed, generator_version, coord)` must produce the same generated blocks.
8. legacy APIs remain available only as a migration bridge; new generator work should not add new behavior to the old atlas-cell path.

## Submodules

- `graph.md`: Voronoi graph ownership and graph-region coordinate contract
- `field.md`: continuous blended field sampling contract
- `hydrology.md`: graph-first watershed and river-edge contract
- `pipeline.md`: graph-first generation stage order and column synthesis scaffolding
- `legacy/legacy.md`: archived previous world module and compatibility bridge

## Current Implementation Notes

- the previous `world` implementation has been moved under `src/world/legacy`
- `src/world/mod.rs` re-exports legacy APIs so the current app and tools can continue compiling during migration
- new graph-first modules are currently scaffold contracts, not a complete generator
- future implementation should replace the legacy generation entrypoint only after graph construction, field sampling, hydrology routing, and heightfield synthesis have focused tests
