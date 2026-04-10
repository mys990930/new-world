use super::atlas::{
    ATLAS_CELL_SIZE_IN_CHUNKS, AtlasArea, AtlasCell, AtlasCoord, AtlasFieldMap, AtlasTuning,
    generate_atlas_fields,
};
use super::chunk::{BlockId, ChunkData};
use super::coord::{CHUNK_EDGE_I32, ChunkCoord, LocalBlockCoord, chunk_local_to_world};
use super::meta::WorldMeta;
use super::registry::BlockRegistry;

pub const SEA_LEVEL_Y: i32 = 0;
pub const WORLD_FLOOR_Y: i32 = -256;
pub const SURFACE_STONE_MIN_DEPTH: i32 = 8;
pub const SURFACE_STONE_MAX_DEPTH: i32 = 16;

#[allow(dead_code)]
pub const FLAT_WORLD_SURFACE_Y: i32 = SEA_LEVEL_Y;

const ATLAS_CELL_SPAN_BLOCKS_I32: i32 = ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32;
const SURFACE_JITTER_SALT: u64 = 0x9511_1100_0000_0001;
const STONE_DEPTH_SALT: u64 = 0x9511_1100_0000_0002;
const MATERIAL_BLEND_SALT: u64 = 0x9511_1100_0000_0003;

#[derive(Debug, Clone, Copy)]
struct GenerationPalette {
    grass: BlockId,
    dirt: BlockId,
    stone: BlockId,
    sand: BlockId,
    gravel: BlockId,
    mud: BlockId,
    snow: BlockId,
    water: BlockId,
}

impl GenerationPalette {
    fn from_registry(registry: &BlockRegistry) -> Self {
        let grass = registry.block_id("grass").unwrap_or(BlockId::GRASS);
        let dirt = registry.block_id("dirt").unwrap_or(BlockId::DIRT);
        let stone = registry.block_id("stone").unwrap_or(BlockId::STONE);

        Self {
            grass,
            dirt,
            stone,
            sand: registry.block_id("sand").unwrap_or(dirt),
            gravel: registry.block_id("gravel").unwrap_or(stone),
            mud: registry.block_id("mud").unwrap_or(dirt),
            snow: registry.block_id("snow").unwrap_or(stone),
            water: registry.block_id("water").unwrap_or(BlockId::AIR),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ColumnAtlasSample {
    landness: f32,
    ocean_distance: f32,
    coast_factor: f32,
    continent_core_factor: f32,
    macro_elevation: f32,
    ridge_factor: f32,
    mountain_mass: f32,
    ruggedness: f32,
    river_flow_potential: f32,
    riverine_factor: f32,
    lake_potential: f32,
    temperature: f32,
    humidity: f32,
    aridity: f32,
    wetness: f32,
    polar_factor: f32,
    alpine_factor: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RiverStage {
    Headwaters,
    Middle,
    Lower,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColumnFillProfile {
    DeepOcean,
    ShallowOcean { river_connected: bool },
    River(RiverStage),
    Coast,
    Desert,
    Frozen,
    SoilWithGrassTop,
}

#[derive(Debug, Clone, Copy)]
struct ColumnRealization {
    surface_y: i32,
    stone_ceiling_y: i32,
    fill_profile: ColumnFillProfile,
}

pub fn generate_chunk(coord: ChunkCoord, meta: &WorldMeta, registry: &BlockRegistry) -> ChunkData {
    let palette = GenerationPalette::from_registry(registry);
    let atlas_fields = generate_chunk_atlas_fields(coord, meta);
    let land_threshold = AtlasTuning::default().normalization.land_threshold;
    let mut chunk = ChunkData::new_empty(coord);

    for local_z in 0..CHUNK_EDGE_I32 as u8 {
        for local_x in 0..CHUNK_EDGE_I32 as u8 {
            let column_origin =
                chunk_local_to_world(coord, LocalBlockCoord::new(local_x, 0, local_z).unwrap());
            let world_x = column_origin.0;
            let world_z = column_origin.2;
            let atlas_sample = sample_column_atlas(&atlas_fields, world_x, world_z);
            let surface_y = compute_surface_y(meta.seed, world_x, world_z, atlas_sample, land_threshold);
            let stone_ceiling_y = compute_stone_ceiling(meta.seed, world_x, world_z, surface_y);
            let fill_profile = classify_fill_profile(atlas_sample, surface_y, land_threshold);
            let realization = ColumnRealization {
                surface_y,
                stone_ceiling_y,
                fill_profile,
            };

            for local_y in 0..CHUNK_EDGE_I32 as u8 {
                let local = LocalBlockCoord::new(local_x, local_y, local_z).unwrap();
                let world_y = chunk_local_to_world(coord, local).1;
                let block =
                    block_for_world_y(world_y, world_x, world_z, meta.seed, realization, palette);
                if block != BlockId::AIR {
                    let _ = chunk.set_block(local, block);
                }
            }
        }
    }

    chunk
}

fn generate_chunk_atlas_fields(coord: ChunkCoord, meta: &WorldMeta) -> AtlasFieldMap {
    let origin = AtlasCoord::new(
        coord.0.div_euclid(ATLAS_CELL_SIZE_IN_CHUNKS as i32),
        coord.2.div_euclid(ATLAS_CELL_SIZE_IN_CHUNKS as i32),
    );
    let area = AtlasArea::new(origin, 2, 2).expect("generation atlas area is valid");
    generate_atlas_fields(meta, area)
}

fn sample_column_atlas(fields: &AtlasFieldMap, world_x: i32, world_z: i32) -> ColumnAtlasSample {
    let atlas_x = world_x.div_euclid(ATLAS_CELL_SPAN_BLOCKS_I32);
    let atlas_z = world_z.div_euclid(ATLAS_CELL_SPAN_BLOCKS_I32);
    let frac_x =
        (world_x.rem_euclid(ATLAS_CELL_SPAN_BLOCKS_I32) as f32 + 0.5) / ATLAS_CELL_SPAN_BLOCKS_I32 as f32;
    let frac_z =
        (world_z.rem_euclid(ATLAS_CELL_SPAN_BLOCKS_I32) as f32 + 0.5) / ATLAS_CELL_SPAN_BLOCKS_I32 as f32;

    let c00 = fields
        .get(AtlasCoord::new(atlas_x, atlas_z))
        .expect("generation atlas sample must exist");
    let c10 = fields
        .get(AtlasCoord::new(atlas_x + 1, atlas_z))
        .expect("generation atlas east sample must exist");
    let c01 = fields
        .get(AtlasCoord::new(atlas_x, atlas_z + 1))
        .expect("generation atlas south sample must exist");
    let c11 = fields
        .get(AtlasCoord::new(atlas_x + 1, atlas_z + 1))
        .expect("generation atlas southeast sample must exist");

    ColumnAtlasSample {
        landness: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| cell.landness),
        ocean_distance: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| cell.ocean_distance),
        coast_factor: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| cell.coast_factor),
        continent_core_factor: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| {
            cell.continent_core_factor
        }),
        macro_elevation: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| cell.macro_elevation),
        ridge_factor: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| cell.ridge_factor),
        mountain_mass: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| cell.mountain_mass),
        ruggedness: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| cell.ruggedness),
        river_flow_potential: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| {
            cell.river_flow_potential
        }),
        riverine_factor: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| cell.riverine_factor),
        lake_potential: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| cell.lake_potential),
        temperature: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| cell.temperature),
        humidity: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| cell.humidity),
        aridity: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| cell.aridity),
        wetness: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| cell.wetness),
        polar_factor: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| cell.polar_factor),
        alpine_factor: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| cell.alpine_factor),
    }
}

fn compute_surface_y(
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
    land_threshold: f32,
) -> i32 {
    let jitter = centered_noise(seed, world_x, world_z, SURFACE_JITTER_SALT);

    if sample.landness < land_threshold {
        let depth_signal = clamp01(sample.ocean_distance * 0.72 + (1.0 - sample.landness) * 0.28);
        let seabed = -lerp_f32(4.0, 30.0, depth_signal) + jitter * 2.0;
        seabed.round().clamp(-40.0, -1.0) as i32
    } else {
        let upland = 2.0
            + sample.macro_elevation * 18.0
            + sample.continent_core_factor * 6.0
            + sample.mountain_mass * 14.0
            + sample.ridge_factor * 5.0
            + sample.ruggedness * 3.0
            - sample.coast_factor * 4.0;
        let river_carve =
            sample.riverine_factor * (2.0 + (1.0 - sample.mountain_mass) * 5.0);
        let alpine_bonus = sample.alpine_factor * 6.0;
        let surface = upland - river_carve + alpine_bonus + jitter * 4.0;
        surface.round().clamp(-2.0, 48.0) as i32
    }
}

fn compute_stone_ceiling(seed: u64, world_x: i32, world_z: i32, surface_y: i32) -> i32 {
    let depth_roll = noise01_2d(seed, world_x, world_z, STONE_DEPTH_SALT);
    let depth = SURFACE_STONE_MIN_DEPTH
        + (depth_roll * (SURFACE_STONE_MAX_DEPTH - SURFACE_STONE_MIN_DEPTH + 1) as f32).floor()
            as i32;
    (surface_y - depth).max(WORLD_FLOOR_Y)
}

fn classify_fill_profile(
    sample: ColumnAtlasSample,
    surface_y: i32,
    land_threshold: f32,
) -> ColumnFillProfile {
    let is_land = sample.landness >= land_threshold;
    let river_connected = sample.riverine_factor > 0.46 || sample.lake_potential > 0.58;

    if !is_land {
        let deep_ocean = surface_y <= -12 || sample.ocean_distance > 0.40;
        return if deep_ocean {
            ColumnFillProfile::DeepOcean
        } else {
            ColumnFillProfile::ShallowOcean { river_connected }
        };
    }

    if river_connected {
        return ColumnFillProfile::River(classify_river_stage(sample));
    }

    if sample.coast_factor > 0.56 && surface_y <= SEA_LEVEL_Y + 6 {
        return ColumnFillProfile::Coast;
    }

    if sample.temperature > 0.56
        && sample.aridity > 0.60
        && sample.humidity < 0.42
        && sample.wetness < 0.38
    {
        return ColumnFillProfile::Desert;
    }

    if sample.alpine_factor > 0.56 || sample.polar_factor > 0.52 {
        return ColumnFillProfile::Frozen;
    }

    ColumnFillProfile::SoilWithGrassTop
}

fn classify_river_stage(sample: ColumnAtlasSample) -> RiverStage {
    if sample.river_flow_potential < 0.24
        || (sample.mountain_mass > 0.40 && sample.macro_elevation > 0.44)
    {
        RiverStage::Headwaters
    } else if sample.river_flow_potential < 0.58 {
        RiverStage::Middle
    } else {
        RiverStage::Lower
    }
}

fn block_for_world_y(
    world_y: i32,
    world_x: i32,
    world_z: i32,
    seed: u64,
    column: ColumnRealization,
    palette: GenerationPalette,
) -> BlockId {
    if world_y < WORLD_FLOOR_Y {
        return BlockId::AIR;
    }

    if world_y <= column.stone_ceiling_y {
        return palette.stone;
    }

    if world_y <= column.surface_y {
        let is_surface = world_y == column.surface_y;
        return fill_block_for_profile(
            column.fill_profile,
            world_x,
            world_y,
            world_z,
            seed,
            is_surface,
            column.surface_y,
            palette,
        );
    }

    if column.surface_y < SEA_LEVEL_Y && world_y <= SEA_LEVEL_Y {
        return palette.water;
    }

    BlockId::AIR
}

fn fill_block_for_profile(
    profile: ColumnFillProfile,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    seed: u64,
    is_surface: bool,
    surface_y: i32,
    palette: GenerationPalette,
) -> BlockId {
    let roll = noise01_3d(seed, world_x, world_y, world_z, MATERIAL_BLEND_SALT);

    match profile {
        ColumnFillProfile::DeepOcean => palette.mud,
        ColumnFillProfile::ShallowOcean { river_connected } => {
            if river_connected {
                if roll < 0.62 {
                    palette.sand
                } else {
                    palette.mud
                }
            } else if roll < 0.22 {
                palette.gravel
            } else if roll < 0.67 {
                palette.sand
            } else {
                palette.mud
            }
        }
        ColumnFillProfile::River(RiverStage::Headwaters) => palette.gravel,
        ColumnFillProfile::River(RiverStage::Middle) => {
            if roll < 0.58 {
                palette.gravel
            } else {
                palette.sand
            }
        }
        ColumnFillProfile::River(RiverStage::Lower) => {
            if roll < 0.52 {
                palette.mud
            } else {
                palette.sand
            }
        }
        ColumnFillProfile::Coast | ColumnFillProfile::Desert => palette.sand,
        ColumnFillProfile::Frozen => palette.snow,
        ColumnFillProfile::SoilWithGrassTop => {
            if is_surface && surface_y >= SEA_LEVEL_Y {
                palette.grass
            } else {
                palette.dirt
            }
        }
    }
}

fn bilerp_cell(
    c00: &AtlasCell,
    c10: &AtlasCell,
    c01: &AtlasCell,
    c11: &AtlasCell,
    tx: f32,
    tz: f32,
    sample: impl Fn(&AtlasCell) -> f32,
) -> f32 {
    bilerp(
        sample(c00),
        sample(c10),
        sample(c01),
        sample(c11),
        tx,
        tz,
    )
}

fn bilerp(a00: f32, a10: f32, a01: f32, a11: f32, tx: f32, tz: f32) -> f32 {
    let north = lerp_f32(a00, a10, tx);
    let south = lerp_f32(a01, a11, tx);
    lerp_f32(north, south, tz)
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn centered_noise(seed: u64, world_x: i32, world_z: i32, salt: u64) -> f32 {
    noise01_2d(seed, world_x, world_z, salt) * 2.0 - 1.0
}

fn noise01_2d(seed: u64, x: i32, z: i32, salt: u64) -> f32 {
    let mut value = seed ^ salt;
    value ^= (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    value ^= (z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    splitmix64(value)
}

fn noise01_3d(seed: u64, x: i32, y: i32, z: i32, salt: u64) -> f32 {
    let mut value = seed ^ salt;
    value ^= (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    value ^= (y as u64).wrapping_mul(0xC6BC_2796_92B5_CC83);
    value ^= (z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    splitmix64(value)
}

fn splitmix64(mut value: u64) -> f32 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    let bits = (value ^ (value >> 31)) >> 40;
    bits as f32 / ((1_u32 << 24) - 1) as f32
}

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_registry() -> BlockRegistry {
        BlockRegistry::load_default().expect("default registry should load")
    }

    #[test]
    fn chunk_generation_is_deterministic() {
        let meta = WorldMeta::new(7);
        let registry = test_registry();

        let a = generate_chunk(ChunkCoord(0, 0, 0), &meta, &registry);
        let b = generate_chunk(ChunkCoord(0, 0, 0), &meta, &registry);

        assert_eq!(a, b);
    }

    #[test]
    fn generation_fills_world_floor_chunk_with_stone() {
        let meta = WorldMeta::new(7);
        let registry = test_registry();
        let stone = registry.block_id("stone").expect("stone block should exist");
        let chunk = generate_chunk(ChunkCoord(0, -8, 0), &meta, &registry);

        assert_eq!(
            chunk.get_block(LocalBlockCoord::new(0, 0, 0).unwrap()),
            Some(stone)
        );
        assert_eq!(
            chunk.get_block(LocalBlockCoord::new(31, 31, 31).unwrap()),
            Some(stone)
        );
    }

    #[test]
    fn soil_profile_uses_grass_top_and_dirt_below() {
        let registry = test_registry();
        let palette = GenerationPalette::from_registry(&registry);
        let column = ColumnRealization {
            surface_y: 4,
            stone_ceiling_y: -6,
            fill_profile: ColumnFillProfile::SoilWithGrassTop,
        };

        assert_eq!(
            block_for_world_y(4, 0, 0, 7, column, palette),
            palette.grass
        );
        assert_eq!(
            block_for_world_y(3, 0, 0, 7, column, palette),
            palette.dirt
        );
        assert_eq!(
            block_for_world_y(-6, 0, 0, 7, column, palette),
            palette.stone
        );
    }

    #[test]
    fn ocean_profile_fills_water_up_to_sea_level() {
        let registry = test_registry();
        let palette = GenerationPalette::from_registry(&registry);
        let column = ColumnRealization {
            surface_y: -5,
            stone_ceiling_y: -12,
            fill_profile: ColumnFillProfile::DeepOcean,
        };

        assert_eq!(
            block_for_world_y(-12, 0, 0, 7, column, palette),
            palette.stone
        );
        assert_eq!(
            block_for_world_y(-5, 0, 0, 7, column, palette),
            palette.mud
        );
        assert_eq!(
            block_for_world_y(-4, 0, 0, 7, column, palette),
            palette.water
        );
        assert_eq!(
            block_for_world_y(0, 0, 0, 7, column, palette),
            palette.water
        );
        assert_eq!(
            block_for_world_y(1, 0, 0, 7, column, palette),
            BlockId::AIR
        );
    }
}
