use super::chunk::BlockId;
use super::coord::{CHUNK_EDGE, CHUNK_EDGE_I32, LocalBlockCoord, WorldBlockCoord};
use super::core::WorldCore;
use super::generation::WORLD_FLOOR_Y;
use super::chunk::ChunkSnapshot;
use super::registry::{BlockDef, BlockMaterialKind, BlockRegistry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopdownCell {
    pub top_y: Option<i32>,
    pub block: BlockId,
}

impl TopdownCell {
    pub const AIR: Self = Self {
        top_y: None,
        block: BlockId::AIR,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopdownColumnScan {
    pub visible: TopdownCell,
    pub top_solid: TopdownCell,
    pub top_water_y: Option<i32>,
    pub water_block_count: u16,
}

impl TopdownColumnScan {
    pub const AIR: Self = Self {
        visible: TopdownCell::AIR,
        top_solid: TopdownCell::AIR,
        top_water_y: None,
        water_block_count: 0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopdownSurfaceRange {
    pub min_y: i32,
    pub max_y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TopdownChunkColumnCoord {
    pub chunk_x: i32,
    pub chunk_z: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopdownChunkColumnPatch {
    coord: TopdownChunkColumnCoord,
    columns: Vec<TopdownColumnScan>,
}

impl TopdownChunkColumnPatch {
    pub fn new(coord: TopdownChunkColumnCoord, columns: Vec<TopdownColumnScan>) -> Self {
        debug_assert_eq!(columns.len(), CHUNK_EDGE * CHUNK_EDGE);
        Self { coord, columns }
    }

    pub fn empty(coord: TopdownChunkColumnCoord) -> Self {
        Self {
            coord,
            columns: vec![TopdownColumnScan::AIR; CHUNK_EDGE * CHUNK_EDGE],
        }
    }

    pub fn coord(&self) -> TopdownChunkColumnCoord {
        self.coord
    }

    pub fn columns(&self) -> &[TopdownColumnScan] {
        &self.columns
    }

    pub fn get(&self, local_x: u32, local_z: u32) -> Option<TopdownColumnScan> {
        let index = chunk_column_index(local_x, local_z)?;
        self.columns.get(index).copied()
    }

    pub fn set(&mut self, local_x: u32, local_z: u32, scan: TopdownColumnScan) -> bool {
        let Some(index) = chunk_column_index(local_x, local_z) else {
            return false;
        };
        if let Some(cell) = self.columns.get_mut(index) {
            *cell = scan;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopdownEdge {
    Left,
    Top,
    Right,
    Bottom,
}

pub fn sample_topdown_columns(
    world: &WorldCore,
    registry: &BlockRegistry,
    min_world_x: i32,
    min_world_z: i32,
    width_blocks: u32,
    height_blocks: u32,
    min_world_y: i32,
    max_world_y: i32,
) -> Vec<TopdownColumnScan> {
    if width_blocks == 0 || height_blocks == 0 {
        return Vec::new();
    }

    let min_world_y = min_world_y.max(WORLD_FLOOR_Y);
    if max_world_y < min_world_y {
        return Vec::new();
    }

    let mut columns =
        Vec::with_capacity(width_blocks as usize * height_blocks as usize);
    for z_offset in 0..height_blocks as usize {
        let world_z = min_world_z + z_offset as i32;
        for x_offset in 0..width_blocks as usize {
            let world_x = min_world_x + x_offset as i32;
            columns.push(sample_single_topdown_column(
                world,
                registry,
                world_x,
                world_z,
                min_world_y,
                max_world_y,
            ));
        }
    }
    columns
}

pub fn sample_single_topdown_column(
    world: &WorldCore,
    registry: &BlockRegistry,
    world_x: i32,
    world_z: i32,
    min_world_y: i32,
    max_world_y: i32,
) -> TopdownColumnScan {
    sample_column_scan(world, registry, world_x, world_z, min_world_y, max_world_y)
}

pub fn sample_topdown_chunk_column(
    registry: &BlockRegistry,
    coord: TopdownChunkColumnCoord,
    chunks: &[ChunkSnapshot],
) -> TopdownChunkColumnPatch {
    let mut ordered_chunks = chunks
        .iter()
        .filter(|chunk| chunk.coord().0 == coord.chunk_x && chunk.coord().2 == coord.chunk_z)
        .collect::<Vec<_>>();
    ordered_chunks.sort_by(|left, right| right.coord().1.cmp(&left.coord().1));

    if ordered_chunks.is_empty() {
        return TopdownChunkColumnPatch::empty(coord);
    }

    let mut columns = Vec::with_capacity(CHUNK_EDGE * CHUNK_EDGE);
    for local_z in 0..CHUNK_EDGE as u32 {
        for local_x in 0..CHUNK_EDGE as u32 {
            columns.push(sample_snapshot_column_scan(
                &ordered_chunks,
                registry,
                local_x,
                local_z,
            ));
        }
    }

    TopdownChunkColumnPatch::new(coord, columns)
}

pub fn topdown_surface_range(
    columns: &[TopdownColumnScan],
) -> Option<TopdownSurfaceRange> {
    let mut top_cells = columns.iter().filter_map(|cell| cell.visible.top_y);
    let first = top_cells.next()?;
    let mut min_y = first;
    let mut max_y = first;

    for top_y in top_cells {
        min_y = min_y.min(top_y);
        max_y = max_y.max(top_y);
    }

    Some(TopdownSurfaceRange { min_y, max_y })
}

pub fn color_topdown_cell(
    cell: TopdownCell,
    registry: &BlockRegistry,
    surface_range: TopdownSurfaceRange,
) -> [u8; 3] {
    let Some(top_y) = cell.top_y else {
        return [18, 22, 28];
    };

    let def = registry.block_or_missing(cell.block);
    let base = block_base_color(def);
    let tint = def.tint;
    let modulated = [
        ((u16::from(base[0]) * u16::from(tint[0])) / 255) as u8,
        ((u16::from(base[1]) * u16::from(tint[1])) / 255) as u8,
        ((u16::from(base[2]) * u16::from(tint[2])) / 255) as u8,
    ];

    let relief_t = if surface_range.max_y > surface_range.min_y {
        (top_y - surface_range.min_y) as f32
            / (surface_range.max_y - surface_range.min_y) as f32
    } else {
        0.5
    };
    let brightness = match def.material {
        BlockMaterialKind::Water => 0.88 + relief_t * 0.16,
        BlockMaterialKind::Emissive => 0.98 + relief_t * 0.08,
        _ => 0.82 + relief_t * 0.22,
    };

    brighten_topdown_color(modulated, brightness)
}

pub fn topdown_edge_strength_for_cell(
    columns: &[TopdownColumnScan],
    width: usize,
    height: usize,
    x: usize,
    z: usize,
    edge: TopdownEdge,
) -> f32 {
    if columns.is_empty() || x >= width || z >= height || width == 0 || height == 0 {
        return 0.0;
    }

    let index = z * width + x;
    if index >= columns.len() {
        return 0.0;
    }

    let cell = columns[index].visible;
    match edge {
        TopdownEdge::Left => {
            edge_strength(cell, x.checked_sub(1).map(|nx| columns[z * width + nx].visible))
        }
        TopdownEdge::Top => {
            edge_strength(cell, z.checked_sub(1).map(|nz| columns[nz * width + x].visible))
        }
        TopdownEdge::Right => {
            let right = if x + 1 < width {
                Some(columns[z * width + (x + 1)].visible)
            } else {
                None
            };
            edge_strength(cell, right)
        }
        TopdownEdge::Bottom => {
            let bottom = if z + 1 < height {
                Some(columns[(z + 1) * width + x].visible)
            } else {
                None
            };
            edge_strength(cell, bottom)
        }
    }
}

pub fn topdown_outline_strength(
    columns: &[TopdownColumnScan],
    width: usize,
    height: usize,
    x: usize,
    z: usize,
    local_x: u32,
    local_y: u32,
    pixels_per_block: u32,
) -> f32 {
    if pixels_per_block <= 1 {
        return 0.0;
    }

    let mut strength = 0.0_f32;
    if local_x == 0 {
        strength = strength.max(topdown_edge_strength_for_cell(
            columns,
            width,
            height,
            x,
            z,
            TopdownEdge::Left,
        ));
    }
    if local_y == 0 {
        strength = strength.max(topdown_edge_strength_for_cell(
            columns,
            width,
            height,
            x,
            z,
            TopdownEdge::Top,
        ));
    }
    if local_x + 1 == pixels_per_block {
        strength = strength.max(topdown_edge_strength_for_cell(
            columns,
            width,
            height,
            x,
            z,
            TopdownEdge::Right,
        ));
    }
    if local_y + 1 == pixels_per_block {
        strength = strength.max(topdown_edge_strength_for_cell(
            columns,
            width,
            height,
            x,
            z,
            TopdownEdge::Bottom,
        ));
    }

    strength
}

pub fn darken_topdown_color(color: [u8; 3], amount: f32) -> [u8; 3] {
    let factor = (1.0 - amount).clamp(0.0, 1.0);
    brighten_topdown_color(color, factor)
}

fn sample_column_scan(
    world: &WorldCore,
    registry: &BlockRegistry,
    world_x: i32,
    world_z: i32,
    min_world_y: i32,
    max_world_y: i32,
) -> TopdownColumnScan {
    let mut scan = TopdownColumnScan::AIR;

    for world_y in (min_world_y..=max_world_y).rev() {
        let Some(block) = world.get_block(WorldBlockCoord(world_x, world_y, world_z)) else {
            continue;
        };
        if block.is_air() {
            continue;
        }

        accumulate_column_hit(&mut scan, registry, block, world_y);
    }

    scan
}

fn sample_snapshot_column_scan(
    chunks: &[&ChunkSnapshot],
    registry: &BlockRegistry,
    local_x: u32,
    local_z: u32,
) -> TopdownColumnScan {
    let mut scan = TopdownColumnScan::AIR;
    let local_x = local_x as u8;
    let local_z = local_z as u8;

    for chunk in chunks {
        for local_y in (0..CHUNK_EDGE as u8).rev() {
            let local = LocalBlockCoord::new(local_x, local_y, local_z)
                .expect("minimap patch scan should stay in chunk bounds");
            let Some(block) = chunk.get_block(local) else {
                continue;
            };
            if block.is_air() {
                continue;
            }

            let world_y = chunk.coord().1 * CHUNK_EDGE_I32 + i32::from(local_y);
            accumulate_column_hit(&mut scan, registry, block, world_y);
        }
    }

    scan
}

fn accumulate_column_hit(
    scan: &mut TopdownColumnScan,
    registry: &BlockRegistry,
    block: BlockId,
    world_y: i32,
) {
    let block_def = registry.block_or_missing(block);
    if scan.visible.top_y.is_none() {
        scan.visible = TopdownCell {
            top_y: Some(world_y),
            block,
        };
    }
    if scan.top_solid.top_y.is_none() && block_def.solid {
        scan.top_solid = TopdownCell {
            top_y: Some(world_y),
            block,
        };
    }
    if block_def.key == "water" {
        if scan.top_water_y.is_none() {
            scan.top_water_y = Some(world_y);
        }
        scan.water_block_count = scan.water_block_count.saturating_add(1);
    }
}

fn chunk_column_index(local_x: u32, local_z: u32) -> Option<usize> {
    if local_x >= CHUNK_EDGE as u32 || local_z >= CHUNK_EDGE as u32 {
        return None;
    }
    Some(local_x as usize + local_z as usize * CHUNK_EDGE)
}

fn block_base_color(def: &BlockDef) -> [u8; 3] {
    match def.key.as_str() {
        "snow" => [244, 248, 255],
        _ => material_base_color(def.material),
    }
}

fn material_base_color(material: BlockMaterialKind) -> [u8; 3] {
    match material {
        BlockMaterialKind::GenericOpaque => [170, 170, 178],
        BlockMaterialKind::Grass => [110, 162, 82],
        BlockMaterialKind::Soil => [122, 90, 60],
        BlockMaterialKind::Stone => [138, 144, 152],
        BlockMaterialKind::Sand => [208, 190, 126],
        BlockMaterialKind::Foliage => [86, 150, 98],
        BlockMaterialKind::Water => [76, 124, 198],
        BlockMaterialKind::Emissive => [236, 194, 88],
    }
}

fn edge_strength(cell: TopdownCell, neighbor: Option<TopdownCell>) -> f32 {
    match neighbor {
        None => 0.24,
        Some(other) if other == cell => 0.10,
        Some(other) if other.top_y == cell.top_y => 0.14,
        Some(_) => 0.24,
    }
}

fn brighten_topdown_color(color: [u8; 3], factor: f32) -> [u8; 3] {
    [
        scale_channel(color[0], factor),
        scale_channel(color[1], factor),
        scale_channel(color[2], factor),
    ]
}

fn scale_channel(channel: u8, factor: f32) -> u8 {
    (channel as f32 * factor).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::world::{BlockRegistry, ChunkCoord, ChunkData, LocalBlockCoord, WorldMeta};

    #[test]
    fn sample_topdown_columns_finds_top_visible_block() {
        let registry =
            Arc::new(BlockRegistry::load_default().expect("default registry should load"));
        let mut world = WorldCore::new(WorldMeta::default(), registry.clone());
        let mut chunk = ChunkData::new_empty(ChunkCoord(0, 0, 0));
        chunk
            .set_block(LocalBlockCoord::new(0, 0, 0).unwrap(), BlockId::STONE)
            .unwrap();
        chunk
            .set_block(LocalBlockCoord::new(0, 1, 0).unwrap(), BlockId::GRASS)
            .unwrap();
        world.insert_chunk(ChunkCoord(0, 0, 0), chunk);

        let columns = sample_topdown_columns(
            &world,
            registry.as_ref(),
            0,
            0,
            1,
            1,
            0,
            3,
        );

        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0].visible.block, BlockId::GRASS);
        assert_eq!(columns[0].visible.top_y, Some(1));
    }

    #[test]
    fn snow_topdown_color_uses_white_override() {
        let registry =
            BlockRegistry::load_default().expect("default registry should load");
        let snow = registry
            .block(registry.block_id("snow").expect("snow block should exist"))
            .expect("snow definition should exist");

        assert_eq!(block_base_color(snow), [244, 248, 255]);
    }

    #[test]
    fn snapshot_chunk_column_sampling_reads_highest_visible_block() {
        let registry =
            Arc::new(BlockRegistry::load_default().expect("default registry should load"));
        let mut lower = ChunkData::new_empty(ChunkCoord(0, 0, 0));
        lower
            .set_block(LocalBlockCoord::new(0, 0, 0).unwrap(), BlockId::STONE)
            .unwrap();
        let mut upper = ChunkData::new_empty(ChunkCoord(0, 1, 0));
        upper
            .set_block(LocalBlockCoord::new(0, 0, 0).unwrap(), BlockId::GRASS)
            .unwrap();

        let patch = sample_topdown_chunk_column(
            registry.as_ref(),
            TopdownChunkColumnCoord {
                chunk_x: 0,
                chunk_z: 0,
            },
            &[lower.snapshot(), upper.snapshot()],
        );

        let cell = patch.get(0, 0).expect("patch cell should exist");
        assert_eq!(cell.visible.block, BlockId::GRASS);
        assert_eq!(cell.visible.top_y, Some(CHUNK_EDGE_I32));
    }
}
