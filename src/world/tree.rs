use std::fmt;

use super::chunk::BlockId;
use super::coord::WorldBlockCoord;
use super::registry::BlockRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TreeKind {
    PolarTundraShrub,
    BorealTaigaConifer,
    TemperateDeciduous,
    TemperateBirch,
    MediterraneanOlive,
    SwampCypress,
    SavannaAcacia,
    TropicalRainforestJungle,
}

impl TreeKind {
    pub const ALL: [Self; 8] = [
        Self::PolarTundraShrub,
        Self::BorealTaigaConifer,
        Self::TemperateDeciduous,
        Self::TemperateBirch,
        Self::MediterraneanOlive,
        Self::SwampCypress,
        Self::SavannaAcacia,
        Self::TropicalRainforestJungle,
    ];

    pub const fn all() -> &'static [Self] {
        &Self::ALL
    }

    pub const fn key(self) -> &'static str {
        match self {
            Self::PolarTundraShrub => "polar_tundra_shrub",
            Self::BorealTaigaConifer => "boreal_taiga_conifer",
            Self::TemperateDeciduous => "temperate_deciduous",
            Self::TemperateBirch => "temperate_birch",
            Self::MediterraneanOlive => "mediterranean_olive",
            Self::SwampCypress => "swamp_cypress",
            Self::SavannaAcacia => "savanna_acacia",
            Self::TropicalRainforestJungle => "tropical_rainforest_jungle",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::PolarTundraShrub => "Polar / Tundra shrub",
            Self::BorealTaigaConifer => "Boreal / Taiga conifer",
            Self::TemperateDeciduous => "Temperate deciduous",
            Self::TemperateBirch => "Temperate birch",
            Self::MediterraneanOlive => "Mediterranean olive",
            Self::SwampCypress => "Swamp cypress",
            Self::SavannaAcacia => "Savanna acacia",
            Self::TropicalRainforestJungle => "Tropical rainforest / jungle",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        let normalized = key.trim().to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "polar_tundra_shrub" | "polar" | "tundra" | "tundra_shrub" => {
                Some(Self::PolarTundraShrub)
            }
            "boreal_taiga_conifer" | "boreal" | "taiga" | "conifer" | "spruce" | "fir" => {
                Some(Self::BorealTaigaConifer)
            }
            "temperate_deciduous" | "deciduous" | "oak" | "temperate_oak" => {
                Some(Self::TemperateDeciduous)
            }
            "temperate_birch" | "birch" => Some(Self::TemperateBirch),
            "mediterranean_olive" | "mediterranean" | "olive" => Some(Self::MediterraneanOlive),
            "swamp_cypress" | "swamp" | "cypress" => Some(Self::SwampCypress),
            "savanna_acacia" | "savanna" | "acacia" => Some(Self::SavannaAcacia),
            "tropical_rainforest_jungle" | "tropical" | "rainforest" | "jungle" => {
                Some(Self::TropicalRainforestJungle)
            }
            _ => None,
        }
    }

    const fn ordinal(self) -> u64 {
        match self {
            Self::PolarTundraShrub => 1,
            Self::BorealTaigaConifer => 2,
            Self::TemperateDeciduous => 3,
            Self::TemperateBirch => 4,
            Self::MediterraneanOlive => 5,
            Self::SwampCypress => 6,
            Self::SavannaAcacia => 7,
            Self::TropicalRainforestJungle => 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeBlockPalette {
    pub trunk: BlockId,
    pub leaves: BlockId,
    pub root: Option<BlockId>,
    pub vine: Option<BlockId>,
}

impl TreeBlockPalette {
    pub fn resolve_default(
        kind: TreeKind,
        registry: &BlockRegistry,
    ) -> Result<Self, TreePaletteError> {
        let keys = default_palette_keys(kind);
        Ok(Self {
            trunk: required_block(registry, kind, keys.trunk)?,
            leaves: required_block(registry, kind, keys.leaves)?,
            root: optional_block(registry, keys.root),
            vine: optional_block(registry, keys.vine),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreePaletteKeys {
    pub trunk: &'static str,
    pub leaves: &'static str,
    pub root: Option<&'static str>,
    pub vine: Option<&'static str>,
}

pub const fn default_palette_keys(kind: TreeKind) -> TreePaletteKeys {
    match kind {
        TreeKind::PolarTundraShrub => TreePaletteKeys {
            trunk: "polar_tundra_shrub_trunk",
            leaves: "polar_tundra_shrub_leaves",
            root: None,
            vine: None,
        },
        TreeKind::BorealTaigaConifer => TreePaletteKeys {
            trunk: "boreal_taiga_conifer_trunk",
            leaves: "boreal_taiga_conifer_leaves",
            root: None,
            vine: None,
        },
        TreeKind::TemperateDeciduous => TreePaletteKeys {
            trunk: "temperate_deciduous_trunk",
            leaves: "temperate_deciduous_leaves",
            root: None,
            vine: None,
        },
        TreeKind::TemperateBirch => TreePaletteKeys {
            trunk: "temperate_birch_trunk",
            leaves: "temperate_birch_leaves",
            root: None,
            vine: None,
        },
        TreeKind::MediterraneanOlive => TreePaletteKeys {
            trunk: "mediterranean_olive_trunk",
            leaves: "mediterranean_olive_leaves",
            root: None,
            vine: None,
        },
        TreeKind::SwampCypress => TreePaletteKeys {
            trunk: "swamp_cypress_trunk",
            leaves: "swamp_cypress_leaves",
            root: Some("swamp_cypress_root"),
            vine: Some("swamp_cypress_vine"),
        },
        TreeKind::SavannaAcacia => TreePaletteKeys {
            trunk: "savanna_acacia_trunk",
            leaves: "savanna_acacia_leaves",
            root: None,
            vine: None,
        },
        TreeKind::TropicalRainforestJungle => TreePaletteKeys {
            trunk: "tropical_rainforest_jungle_trunk",
            leaves: "tropical_rainforest_jungle_leaves",
            root: Some("tropical_rainforest_jungle_root"),
            vine: Some("tropical_rainforest_jungle_vine"),
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreePaletteError {
    MissingRequiredBlock { kind: TreeKind, key: &'static str },
}

impl fmt::Display for TreePaletteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRequiredBlock { kind, key } => {
                write!(f, "missing required block '{key}' for {}", kind.key())
            }
        }
    }
}

impl std::error::Error for TreePaletteError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeGenRequest {
    pub kind: TreeKind,
    pub origin: WorldBlockCoord,
    pub seed: u64,
    pub palette: TreeBlockPalette,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TreeVoxelRole {
    Trunk,
    Root,
    Branch,
    Leaf,
    Vine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeVoxel {
    pub offset: [i16; 3],
    pub block: BlockId,
    pub role: TreeVoxelRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeBounds {
    pub min: [i16; 3],
    pub max: [i16; 3],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeBlueprint {
    pub kind: TreeKind,
    pub origin: WorldBlockCoord,
    pub voxels: Vec<TreeVoxel>,
    pub bounds: TreeBounds,
}

pub fn generate_tree_blueprint(request: TreeGenRequest) -> TreeBlueprint {
    let mut rng = TreeRng::new(mix_seed(request.seed, request.kind, request.origin));
    let mut builder = TreeBuilder::new(request.kind, request.origin);

    match request.kind {
        TreeKind::PolarTundraShrub => {
            build_polar_tundra_shrub(&mut builder, &mut rng, request.palette)
        }
        TreeKind::BorealTaigaConifer => {
            build_boreal_taiga_conifer(&mut builder, &mut rng, request.palette)
        }
        TreeKind::TemperateDeciduous => {
            build_temperate_deciduous(&mut builder, &mut rng, request.palette)
        }
        TreeKind::TemperateBirch => build_temperate_birch(&mut builder, &mut rng, request.palette),
        TreeKind::MediterraneanOlive => {
            build_mediterranean_olive(&mut builder, &mut rng, request.palette)
        }
        TreeKind::SwampCypress => build_swamp_cypress(&mut builder, &mut rng, request.palette),
        TreeKind::SavannaAcacia => build_savanna_acacia(&mut builder, &mut rng, request.palette),
        TreeKind::TropicalRainforestJungle => {
            build_tropical_rainforest_jungle(&mut builder, &mut rng, request.palette)
        }
    }

    builder.finish()
}

fn build_polar_tundra_shrub(
    builder: &mut TreeBuilder,
    rng: &mut TreeRng,
    palette: TreeBlockPalette,
) {
    let height = 3 + rng.range_i16(0, 2);
    let lean = direction4(rng.next_u32());
    let bend_at = 1 + rng.range_i16(0, 1);
    let mut x = 0;
    let mut z = 0;

    for y in 0..height {
        if y >= bend_at {
            x += lean.0;
            z += lean.1;
        }
        builder.put([x, y, z], palette.trunk, TreeVoxelRole::Trunk);
        if y > 0 && rng.chance(1, 3) {
            let side = direction4(rng.next_u32());
            builder.put(
                [x + side.0, y, z + side.1],
                palette.trunk,
                TreeVoxelRole::Branch,
            );
        }
    }

    for center in [[x, height, z], [x - lean.0, height - 1, z - lean.1]] {
        add_blob(builder, rng, center, 2, 1, palette.leaves, 70);
        builder.put(center, palette.leaves, TreeVoxelRole::Leaf);
    }
}

fn build_boreal_taiga_conifer(
    builder: &mut TreeBuilder,
    rng: &mut TreeRng,
    palette: TreeBlockPalette,
) {
    let height = 9 + rng.range_i16(0, 5);
    for y in 0..height {
        builder.put([0, y, 0], palette.trunk, TreeVoxelRole::Trunk);
    }

    for y in 2..=height {
        let distance_from_top = height - y;
        let mut radius = 1 + distance_from_top / 3;
        radius = radius.clamp(1, 4);
        if y % 2 == 0 {
            radius += 1;
        }
        add_conifer_layer(builder, rng, y, radius, palette.leaves);
    }

    builder.put([0, height + 1, 0], palette.leaves, TreeVoxelRole::Leaf);
}

fn build_temperate_deciduous(
    builder: &mut TreeBuilder,
    rng: &mut TreeRng,
    palette: TreeBlockPalette,
) {
    let height = 6 + rng.range_i16(0, 3);
    for y in 0..height {
        builder.put([0, y, 0], palette.trunk, TreeVoxelRole::Trunk);
        if y < 3 && rng.chance(1, 2) {
            builder.put([1, y, 0], palette.trunk, TreeVoxelRole::Trunk);
        }
    }

    let branch_y = height - 2;
    for dir in shuffled_dirs4(rng.next_u32()) {
        let length = 2 + rng.range_i16(0, 1);
        for step in 1..=length {
            builder.put(
                [dir.0 * step, branch_y + step / 2, dir.1 * step],
                palette.trunk,
                TreeVoxelRole::Branch,
            );
        }
    }

    add_blob(builder, rng, [0, height, 0], 4, 3, palette.leaves, 82);
    add_blob(builder, rng, [1, height - 1, -1], 3, 2, palette.leaves, 78);
}

fn build_temperate_birch(builder: &mut TreeBuilder, rng: &mut TreeRng, palette: TreeBlockPalette) {
    let height = 7 + rng.range_i16(0, 4);
    let lean = if rng.chance(1, 3) {
        direction4(rng.next_u32())
    } else {
        (0, 0)
    };
    let mut x = 0;
    let mut z = 0;
    for y in 0..height {
        if y > height / 2 && y % 3 == 0 {
            x += lean.0;
            z += lean.1;
        }
        builder.put([x, y, z], palette.trunk, TreeVoxelRole::Trunk);
    }

    add_blob(builder, rng, [x, height, z], 3, 3, palette.leaves, 72);
    add_blob(builder, rng, [x, height + 1, z], 2, 2, palette.leaves, 76);
}

fn build_mediterranean_olive(
    builder: &mut TreeBuilder,
    rng: &mut TreeRng,
    palette: TreeBlockPalette,
) {
    let height = 4 + rng.range_i16(0, 2);
    for y in 0..height {
        builder.put([0, y, 0], palette.trunk, TreeVoxelRole::Trunk);
        if y < 2 {
            builder.put([1, y, 0], palette.trunk, TreeVoxelRole::Trunk);
        }
    }

    for dir in shuffled_dirs4(rng.next_u32()) {
        let length = 3 + rng.range_i16(0, 2);
        for step in 1..=length {
            builder.put(
                [dir.0 * step, height - 1 + step / 3, dir.1 * step],
                palette.trunk,
                TreeVoxelRole::Branch,
            );
        }
        add_blob(
            builder,
            rng,
            [dir.0 * length, height, dir.1 * length],
            3,
            1,
            palette.leaves,
            52,
        );
    }
}

fn build_swamp_cypress(builder: &mut TreeBuilder, rng: &mut TreeRng, palette: TreeBlockPalette) {
    let height = 8 + rng.range_i16(0, 4);
    let root_block = palette.root.unwrap_or(palette.trunk);
    for y in 0..height {
        builder.put([0, y, 0], palette.trunk, TreeVoxelRole::Trunk);
        if y < height - 2 && y % 2 == 0 {
            builder.put([1, y, 0], palette.trunk, TreeVoxelRole::Trunk);
        }
    }

    for dir in dirs8() {
        let length = if dir.0 == 0 || dir.1 == 0 { 3 } else { 2 };
        for step in 1..=length {
            builder.put(
                [dir.0 * step, 0, dir.1 * step],
                root_block,
                TreeVoxelRole::Root,
            );
            if step <= 2 {
                builder.put(
                    [dir.0 * step, 1, dir.1 * step],
                    root_block,
                    TreeVoxelRole::Root,
                );
            }
        }
    }

    add_blob(builder, rng, [0, height, 0], 4, 2, palette.leaves, 68);
    add_blob(builder, rng, [1, height - 1, 1], 3, 2, palette.leaves, 64);
    if let Some(vine) = palette.vine {
        add_hanging_vines(builder, rng, height - 1, 4, vine, 7);
    }
}

fn build_savanna_acacia(builder: &mut TreeBuilder, rng: &mut TreeRng, palette: TreeBlockPalette) {
    let height = 7 + rng.range_i16(0, 3);
    for y in 0..height {
        builder.put([0, y, 0], palette.trunk, TreeVoxelRole::Trunk);
    }

    for dir in shuffled_dirs4(rng.next_u32()).into_iter().take(3) {
        let length = 3 + rng.range_i16(0, 2);
        for step in 1..=length {
            builder.put(
                [dir.0 * step, height - 1 + step / 3, dir.1 * step],
                palette.trunk,
                TreeVoxelRole::Branch,
            );
        }
    }

    add_flat_canopy(builder, rng, [0, height + 1, 0], 5, palette.leaves, 64);
    add_flat_canopy(builder, rng, [1, height, -1], 4, palette.leaves, 54);
}

fn build_tropical_rainforest_jungle(
    builder: &mut TreeBuilder,
    rng: &mut TreeRng,
    palette: TreeBlockPalette,
) {
    let height = 13 + rng.range_i16(0, 6);
    let root_block = palette.root.unwrap_or(palette.trunk);
    for y in 0..height {
        builder.put([0, y, 0], palette.trunk, TreeVoxelRole::Trunk);
        if y < height - 3 {
            builder.put([1, y, 0], palette.trunk, TreeVoxelRole::Trunk);
            if y % 2 == 0 {
                builder.put([0, y, 1], palette.trunk, TreeVoxelRole::Trunk);
            }
        }
    }

    for dir in dirs8() {
        for step in 1..=2 {
            builder.put(
                [dir.0 * step, step - 1, dir.1 * step],
                root_block,
                TreeVoxelRole::Root,
            );
        }
    }

    add_blob(builder, rng, [0, height, 0], 5, 3, palette.leaves, 86);
    add_blob(builder, rng, [2, height - 3, 1], 4, 2, palette.leaves, 76);
    add_blob(builder, rng, [-2, height - 6, -1], 3, 2, palette.leaves, 72);

    if let Some(vine) = palette.vine {
        add_hanging_vines(builder, rng, height, 5, vine, 12);
    }
}

fn add_blob(
    builder: &mut TreeBuilder,
    rng: &mut TreeRng,
    center: [i16; 3],
    radius_xz: i16,
    radius_y: i16,
    block: BlockId,
    fill_percent: u32,
) {
    let rx = radius_xz.max(1);
    let ry = radius_y.max(1);
    let rx2 = i32::from(rx * rx);
    let ry2 = i32::from(ry * ry);
    for y in -ry..=ry {
        for z in -rx..=rx {
            for x in -rx..=rx {
                let normalized = i32::from(x * x + z * z) * ry2 + i32::from(y * y) * rx2;
                if normalized > rx2 * ry2 {
                    continue;
                }
                if rng.percent() > fill_percent && normalized > (rx2 * ry2) / 3 {
                    continue;
                }
                builder.put(
                    [center[0] + x, center[1] + y, center[2] + z],
                    block,
                    TreeVoxelRole::Leaf,
                );
            }
        }
    }
}

fn add_conifer_layer(
    builder: &mut TreeBuilder,
    rng: &mut TreeRng,
    y: i16,
    radius: i16,
    block: BlockId,
) {
    for z in -radius..=radius {
        for x in -radius..=radius {
            let distance = x.abs() + z.abs();
            if distance > radius + 1 {
                continue;
            }
            if distance == 0 && y % 2 == 0 {
                continue;
            }
            if distance > radius && rng.chance(1, 2) {
                continue;
            }
            builder.put([x, y, z], block, TreeVoxelRole::Leaf);
        }
    }
}

fn add_flat_canopy(
    builder: &mut TreeBuilder,
    rng: &mut TreeRng,
    center: [i16; 3],
    radius: i16,
    block: BlockId,
    fill_percent: u32,
) {
    for z in -radius..=radius {
        for x in -radius..=radius {
            let d2 = i32::from(x * x + z * z);
            if d2 > i32::from(radius * radius) {
                continue;
            }
            if rng.percent() > fill_percent && d2 > i32::from(radius * radius / 2) {
                continue;
            }
            let y = center[1] + if d2 < 4 { 1 } else { rng.range_i16(0, 1) };
            builder.put(
                [center[0] + x, y, center[2] + z],
                block,
                TreeVoxelRole::Leaf,
            );
        }
    }
}

fn add_hanging_vines(
    builder: &mut TreeBuilder,
    rng: &mut TreeRng,
    canopy_y: i16,
    radius: i16,
    block: BlockId,
    attempts: usize,
) {
    for _ in 0..attempts {
        let x = rng.range_i16(-radius, radius);
        let z = rng.range_i16(-radius, radius);
        if x.abs() + z.abs() < radius / 2 {
            continue;
        }
        let length = 2 + rng.range_i16(0, 5);
        for step in 0..length {
            builder.put([x, canopy_y - step, z], block, TreeVoxelRole::Vine);
        }
    }
}

struct TreeBuilder {
    kind: TreeKind,
    origin: WorldBlockCoord,
    voxels: Vec<TreeVoxel>,
}

impl TreeBuilder {
    fn new(kind: TreeKind, origin: WorldBlockCoord) -> Self {
        Self {
            kind,
            origin,
            voxels: Vec::with_capacity(256),
        }
    }

    fn put(&mut self, offset: [i16; 3], block: BlockId, role: TreeVoxelRole) {
        if let Some(existing) = self.voxels.iter_mut().find(|voxel| voxel.offset == offset) {
            if role_priority(role) < role_priority(existing.role) {
                *existing = TreeVoxel {
                    offset,
                    block,
                    role,
                };
            }
            return;
        }

        self.voxels.push(TreeVoxel {
            offset,
            block,
            role,
        });
    }

    fn finish(mut self) -> TreeBlueprint {
        self.voxels.sort_by_key(|voxel| {
            (
                role_priority(voxel.role),
                voxel.offset[1],
                voxel.offset[0],
                voxel.offset[2],
            )
        });
        let bounds = bounds_for_voxels(&self.voxels);
        TreeBlueprint {
            kind: self.kind,
            origin: self.origin,
            voxels: self.voxels,
            bounds,
        }
    }
}

fn bounds_for_voxels(voxels: &[TreeVoxel]) -> TreeBounds {
    let mut min = [0, 0, 0];
    let mut max = [0, 0, 0];
    for (index, voxel) in voxels.iter().enumerate() {
        if index == 0 {
            min = voxel.offset;
            max = voxel.offset;
        } else {
            for axis in 0..3 {
                min[axis] = min[axis].min(voxel.offset[axis]);
                max[axis] = max[axis].max(voxel.offset[axis]);
            }
        }
    }
    TreeBounds { min, max }
}

fn role_priority(role: TreeVoxelRole) -> u8 {
    match role {
        TreeVoxelRole::Trunk => 0,
        TreeVoxelRole::Root => 1,
        TreeVoxelRole::Branch => 2,
        TreeVoxelRole::Leaf => 3,
        TreeVoxelRole::Vine => 4,
    }
}

fn required_block(
    registry: &BlockRegistry,
    kind: TreeKind,
    key: &'static str,
) -> Result<BlockId, TreePaletteError> {
    registry
        .block_id(key)
        .ok_or(TreePaletteError::MissingRequiredBlock { kind, key })
}

fn optional_block(registry: &BlockRegistry, key: Option<&'static str>) -> Option<BlockId> {
    key.and_then(|key| registry.block_id(key))
}

fn mix_seed(seed: u64, kind: TreeKind, origin: WorldBlockCoord) -> u64 {
    let mut value = seed ^ kind.ordinal().wrapping_mul(0x9e37_79b9_7f4a_7c15);
    value ^= (origin.0 as i64 as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= (origin.1 as i64 as u64).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^= (origin.2 as i64 as u64).wrapping_mul(0xd6e8_feb8_6659_fd93);
    splitmix64(value)
}

struct TreeRng {
    state: u64,
}

impl TreeRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        (splitmix64(self.state) >> 32) as u32
    }

    fn percent(&mut self) -> u32 {
        self.next_u32() % 100
    }

    fn chance(&mut self, numerator: u32, denominator: u32) -> bool {
        denominator != 0 && self.next_u32() % denominator < numerator
    }

    fn range_i16(&mut self, min: i16, max: i16) -> i16 {
        if min >= max {
            return min;
        }
        let span = i32::from(max - min + 1) as u32;
        min + (self.next_u32() % span) as i16
    }
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn direction4(value: u32) -> (i16, i16) {
    match value % 4 {
        0 => (1, 0),
        1 => (-1, 0),
        2 => (0, 1),
        _ => (0, -1),
    }
}

fn shuffled_dirs4(value: u32) -> [(i16, i16); 4] {
    let dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    match value % 4 {
        0 => dirs,
        1 => [dirs[1], dirs[3], dirs[0], dirs[2]],
        2 => [dirs[2], dirs[0], dirs[3], dirs[1]],
        _ => [dirs[3], dirs[2], dirs[1], dirs[0]],
    }
}

fn dirs8() -> [(i16, i16); 8] {
    [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (-1, 1),
        (1, -1),
        (-1, -1),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> TreeBlockPalette {
        TreeBlockPalette {
            trunk: BlockId::new(32),
            leaves: BlockId::new(33),
            root: Some(BlockId::new(34)),
            vine: Some(BlockId::new(35)),
        }
    }

    #[test]
    fn every_tree_kind_generates_voxels_deterministically() {
        for kind in TreeKind::all() {
            let request = TreeGenRequest {
                kind: *kind,
                origin: WorldBlockCoord(10, 20, -30),
                seed: 42,
                palette: palette(),
            };

            let a = generate_tree_blueprint(request);
            let b = generate_tree_blueprint(request);

            assert_eq!(a, b);
            assert!(!a.voxels.is_empty());
            assert!(
                a.voxels
                    .iter()
                    .any(|voxel| voxel.role == TreeVoxelRole::Trunk)
            );
            assert!(
                a.voxels
                    .iter()
                    .any(|voxel| voxel.role == TreeVoxelRole::Leaf)
            );
        }
    }

    #[test]
    fn different_seeds_vary_the_same_tree_kind() {
        let a = generate_tree_blueprint(TreeGenRequest {
            kind: TreeKind::TemperateDeciduous,
            origin: WorldBlockCoord(0, 0, 0),
            seed: 1,
            palette: palette(),
        });
        let b = generate_tree_blueprint(TreeGenRequest {
            kind: TreeKind::TemperateDeciduous,
            origin: WorldBlockCoord(0, 0, 0),
            seed: 2,
            palette: palette(),
        });

        assert_ne!(a.voxels, b.voxels);
    }

    #[test]
    fn woody_voxels_are_ordered_before_leaves_and_vines() {
        let tree = generate_tree_blueprint(TreeGenRequest {
            kind: TreeKind::SwampCypress,
            origin: WorldBlockCoord(0, 0, 0),
            seed: 3,
            palette: palette(),
        });

        let first_leaf = tree
            .voxels
            .iter()
            .position(|voxel| matches!(voxel.role, TreeVoxelRole::Leaf | TreeVoxelRole::Vine))
            .expect("tree should have leaves or vines");
        assert!(tree.voxels[..first_leaf].iter().all(|voxel| matches!(
            voxel.role,
            TreeVoxelRole::Trunk | TreeVoxelRole::Root | TreeVoxelRole::Branch
        )));
    }

    #[test]
    fn default_palettes_resolve_from_block_registry() {
        let registry = BlockRegistry::load_default().expect("default registry should load");

        for kind in TreeKind::all() {
            let palette = TreeBlockPalette::resolve_default(*kind, &registry)
                .expect("tree palette should resolve");
            assert!(registry.block(palette.trunk).is_some());
            assert!(registry.block(palette.leaves).is_some());
        }
    }
}
