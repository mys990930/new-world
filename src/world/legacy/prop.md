# prop

## Role

- Own file-backed microvoxel prop definitions for small surface features such as stones.
- Provide a world-side CPU mesh bake path that turns a prop placement into ordinary `CpuMesh`.

## Responsibilities

- Load prop catalogs from `assets/props/index.toml`.
- Parse prop definitions authored in one-block `16x16x16` micro units.
- Resolve prop texture keys through `BlockRegistry`.
- Resolve prop material kind into `BlockMaterialKind`.
- Bake microvoxel cuboids into renderer-ready world `CpuMesh` vertices.
- Keep prop placement deterministic and renderer-independent.

## Non-Responsibilities

- Deciding where props spawn.
- Vegetation density, biome selection, or slope masks.
- Mutating `ChunkData`.
- GPU upload or renderer resource creation.
- Physics/collision ownership for sub-block shapes.

## Owned Data

- `MicrovoxelPropCatalog`
- `MicrovoxelPropDef`
- `MicrovoxelCuboid`
- `MicrovoxelPropPlacement`
- `MicrovoxelPropError`

## Asset Contract

Prop files use micro units where one world block is `16` units wide, high, and deep.

```toml
key = "rock_stacked_2x2x1_plus_1"
texture = "rock"
material = "stone"
tint = [136, 134, 124, 255]

[[cuboids]]
x = 7
y = 0
z = 7
w = 2
h = 1
d = 2

[[cuboids]]
x = 8
y = 1
z = 8
w = 1
h = 1
d = 1
```

`x/y/z/w/h/d` must fit inside `0..16` micro units. The prop key must be unique within a catalog.

## Public Interface

```rust
MicrovoxelPropCatalog::load_default() -> Result<MicrovoxelPropCatalog, MicrovoxelPropError>
MicrovoxelPropCatalog::load_from_path(path) -> Result<MicrovoxelPropCatalog, MicrovoxelPropError>
MicrovoxelPropCatalog::prop(key) -> Option<&MicrovoxelPropDef>

build_microvoxel_prop_mesh(
    prop: &MicrovoxelPropDef,
    placement: MicrovoxelPropPlacement,
    registry: &BlockRegistry,
) -> Result<CpuMesh, MicrovoxelPropError>
```

## Invariants

1. Prop definitions are world-owned data, not renderer-owned assets.
2. Prop mesh bake emits ordinary `CpuMesh`; the renderer only sees vertices, texture layers, and material kinds.
3. One prop definition is authored inside one block cell. Larger features should compose multiple placements or use a future blueprint.
4. Placement is world-space and deterministic. Future vegetation placement should emit prop placement references, not GPU objects.
5. Prop mesh bake does not mutate chunks and does not change terrain block identity.

## Current Implementation Notes

- The launch catalog contains four rock variants: `1x1x1`, `2x2x1`, `2x2x1 + 1x1x1`, and a low clustered stone.
- Prop cuboids currently emit their own faces directly. This is acceptable for sparse small stones; later batching can merge adjacent cuboids or cache baked shape meshes.
- Stone props use existing block texture layers such as `rock` and `exposed_rock`, so no renderer texture-array contract changes are needed.
