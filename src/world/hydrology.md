# hydrology

## Role

- define the graph-first hydrology contract built from Voronoi corners and selected Voronoi edges
- use polygon boundaries as candidate drainage structure without making every boundary a visible river

## Responsibilities

- represent watersheds, drainage nodes, and river segments over the macro graph
- classify selected graph edges as divides, headwaters, tributaries, trunks, floodplains, or outlets
- carry flow accumulation and downstream progress for later heightfield and water-surface synthesis

## Non-Responsibilities

- generating the base noise heightfield
- carving final voxel channels
- choosing sediment or surface material
- mutating live world storage

## Invariants

1. Voronoi edges are hydrology candidates, not automatic rivers.
2. selected river segments must follow descending or outlet-carved graph logic.
3. local minima are resolved explicitly as lakes, sinks, or carved outlets.
4. final river geometry must use spline and domain-warped realization rather than raw straight graph edges.

## Current Status

- this module is a scaffolded graph data contract
- future implementation should solve watershed routing after continuous field generation and before heightfield synthesis
