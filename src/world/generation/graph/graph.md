# graph

## Role

- own the Voronoi-style macro graph contract for graph-first world generation
- define stable graph ownership regions that can be generated on demand without making chunk or square atlas cells visible in terrain
- provide typed ids and graph patch containers for sites, corners, and edges

## Responsibilities

- map world-space block coordinates to graph cache regions
- represent polygon sites that carry regional semantic seeds such as temperature, hydration, elevation bias, continentality, and ruggedness
- represent corners and edges that hydrology and boundary blending can consume
- keep graph ownership deterministic from `WorldMeta.seed`, `WorldMeta.generator_version`, and `GraphRegionCoord`

## Non-Responsibilities

- final biome selection
- river path solving
- noise synthesis or final heightfield output
- block placement
- live chunk storage mutation

## Invariants

1. graph regions are cache and ownership units only; they must not become visible terrain units.
2. Voronoi sites, corners, and edges are macro semantic guides, not final block-space shapes.
3. neighboring graph regions must be generated with enough padding that sites and edges crossing ownership boundaries remain stable.
4. negative world coordinates use Euclidean division so graph region ownership is stable in every quadrant.

## Current Status

- this module is a scaffolded data contract and small coordinate helper layer
- actual site generation, graph relaxation, Delaunay/Voronoi construction, and padded graph patch assembly remain future implementation work
