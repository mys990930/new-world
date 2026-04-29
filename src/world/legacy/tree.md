# tree

## Role

- provide deterministic voxel tree blueprints for later chunk generation
- own tree form rules without mutating loaded world storage

## Responsibilities

- define supported climate tree kinds
- define per-kind trunk, branch, leaf, root, and vine voxel rules
- vary each tree deterministically from a seed while keeping the same rule family
- emit compact relative voxel lists that generation can place into chunks later
- resolve default per-tree block palettes through `BlockRegistry`

## Non-Responsibilities

- deciding where trees spawn
- deciding forest density
- mutating `WorldCore`
- writing blocks into live chunks
- owning ecology, seasonal growth, or biome classification
- loading PNG textures or renderer resources

## Owned Data

- `TreeKind`
- `TreeBlockPalette`
- `TreeGenRequest`
- `TreeVoxelRole`
- `TreeVoxel`
- `TreeBounds`
- `TreeBlueprint`
- `TreePaletteError`

## Public Interface

```rust
TreeKind::from_key(key: &str) -> Option<TreeKind>
TreeKind::key(self) -> &'static str
TreeKind::all() -> &'static [TreeKind]

TreeBlockPalette::resolve_default(kind: TreeKind, registry: &BlockRegistry)
    -> Result<TreeBlockPalette, TreePaletteError>

generate_tree_blueprint(request: TreeGenRequest) -> TreeBlueprint
```

## Tree Kinds

- `PolarTundraShrub`: low, crooked sparse-leaf shrub form, roughly 1.0-2.0m tall.
- `BorealTaigaConifer`: pointed conifer with a short visible lower trunk and narrow stacked triangular leaf shelves, roughly 7.0-11.5m tall.
- `TemperateDeciduous`: broad oak-like straight 1x1 or 2x2 trunk with rounded crown, roughly 5.0-8.0m tall.
- `TemperateBirch`: pale straight trunk with light oval crown, roughly 6.0-10.0m tall.
- `MediterraneanOlive`: low, moderately spreading, sparse dry-climate crown, roughly 3.0-5.5m tall.
- `SwampCypress`: wetland tree with straight 1x1 or 2x2 trunk, irregular exposed roots, and leaf-attached hanging vines, roughly 7.0-11.5m tall.
- `SavannaAcacia`: tall trunk with sparse branches and umbrella crown, roughly 5.5-9.0m tall.
- `TropicalRainforestJungle`: very tall dense tree with layered crown, irregular buttress roots, and leaf-attached vines, roughly 12.0-19.0m tall.

## Scale

- Tree rules emit block offsets, but their dimensions are authored against the world coordinate contract in `coord.md`.
- The current world scale is `1 block = 0.5m`, so tree heights and crown radii use two block units per meter.
- Rule constants should be kept in block counts that correspond to the intended physical tree size, not in abstract visual units.

## Invariants

1. The same `(kind, origin, seed, palette)` always emits the same blueprint.
2. The tree module never stores a tree in the live world.
3. Output voxels are relative to `TreeGenRequest.origin`.
4. Trunk/root/branch voxels are emitted before leaves and vines so later generation can apply woody support first.
5. Leaf and vine voxels never replace trunk/root/branch voxels at the same offset.
6. Generation rules use bounded integer loops and small hash/RNG helpers so dozens of trees per chunk remain cheap.
7. Block meaning comes from `BlockId` / `BlockRegistry`; tree rules do not parse asset files directly.

## Preview

`src/bin/tree_preview.rs` renders a single generated tree blueprint through the existing world meshing and offscreen renderer path. It is a diagnostic tool only and does not change the generation pipeline.
