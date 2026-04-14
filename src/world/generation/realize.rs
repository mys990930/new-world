use super::context::{
    ColumnAtlasSample, ColumnFillProfile, ColumnRealization, GenerationPalette, RiverStage,
};
use super::noise::{
    MATERIAL_BLEND_SALT, STONE_DEPTH_SALT, centered_fbm, clamp01, hash01_2d, hash01_3d,
    lerp_f32,
};
use super::sampler::generate_chunk_atlas_fields;
use super::surface::build_chunk_surface_field;
use super::super::atlas::AtlasTuning;
use super::super::chunk::{BlockId, ChunkData};
use super::super::coord::{CHUNK_EDGE_I32, ChunkCoord, LocalBlockCoord, chunk_local_to_world};
use super::super::meta::WorldMeta;
use super::super::registry::BlockRegistry;
use super::{SEA_LEVEL_Y, WORLD_FLOOR_Y};

const SURFACE_STONE_MIN_DEPTH: i32 = 8;
const SURFACE_STONE_MAX_DEPTH: i32 = 16;
const RIVER_CHANNEL_PRIMARY_SALT: u64 = MATERIAL_BLEND_SALT.wrapping_add(0x4100);
const RIVER_CHANNEL_SECONDARY_SALT: u64 = MATERIAL_BLEND_SALT.wrapping_add(0x4101);

#[derive(Debug, Clone, Copy)]
struct HydrologyRealization {
    surface_y: f32,
    water_top_y: Option<f32>,
}

pub fn generate_chunk(coord: ChunkCoord, meta: &WorldMeta, registry: &BlockRegistry) -> ChunkData {
    let palette = GenerationPalette::from_registry(registry);
    let atlas_fields = generate_chunk_atlas_fields(coord, meta);
    let surface_field = build_chunk_surface_field(coord, meta, &atlas_fields);
    let land_threshold = AtlasTuning::default().normalization.land_threshold;
    let mut chunk = ChunkData::new_empty(coord);

    for local_z in 0..CHUNK_EDGE_I32 as u8 {
        for local_x in 0..CHUNK_EDGE_I32 as u8 {
            let column_origin =
                chunk_local_to_world(coord, LocalBlockCoord::new(local_x, 0, local_z).unwrap());
            let world_x = column_origin.0;
            let world_z = column_origin.2;
            let surface_column = surface_field.column(local_x, local_z);
            let atlas_sample = surface_column.atlas_sample;
            let profile = surface_column.profile;
            let base_surface_y = surface_column.surface_y;
            let fill_profile =
                classify_fill_profile(atlas_sample, base_surface_y.round() as i32, land_threshold, profile);
            let hydrology = apply_hydrology(
                meta.seed,
                world_x,
                world_z,
                atlas_sample,
                base_surface_y,
                fill_profile,
                land_threshold,
                surface_column.local_concavity,
            );
            let surface_y = hydrology.surface_y.round() as i32;
            let water_top_y = hydrology
                .water_top_y
                .map(|water_top_y| water_top_y.floor() as i32)
                .filter(|water_top_y| *water_top_y > surface_y);
            let stone_ceiling_y =
                compute_stone_ceiling(meta.seed, world_x, world_z, surface_y);
            let realization = ColumnRealization {
                surface_y,
                stone_ceiling_y,
                water_top_y,
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

fn compute_stone_ceiling(seed: u64, world_x: i32, world_z: i32, surface_y: i32) -> i32 {
    let depth_roll = hash01_2d(seed, world_x, world_z, STONE_DEPTH_SALT);
    let depth = SURFACE_STONE_MIN_DEPTH
        + (depth_roll * (SURFACE_STONE_MAX_DEPTH - SURFACE_STONE_MIN_DEPTH + 1) as f32).floor()
            as i32;
    (surface_y - depth).max(WORLD_FLOOR_Y)
}

pub(super) fn classify_fill_profile(
    sample: ColumnAtlasSample,
    surface_y: i32,
    land_threshold: f32,
    profile: super::profile::TerrainProfile,
) -> ColumnFillProfile {
    let is_land = sample.landness >= land_threshold;
    let river_connected = (sample.riverine_factor > 0.54 && sample.river_flow_potential > 0.14)
        || sample.river_flow_potential > 0.42
        || sample.lake_potential > 0.66;
    let coast_like = matches!(profile, super::profile::TerrainProfile::Coast)
        || (sample.coast_factor > 0.40
            && surface_y <= SEA_LEVEL_Y + 8
            && sample.mountain_mass < 0.38);

    if !is_land {
        return match profile {
            super::profile::TerrainProfile::DeepOcean => ColumnFillProfile::DeepOcean,
            _ => ColumnFillProfile::ShallowOcean { river_connected },
        };
    }

    if coast_like {
        return ColumnFillProfile::Coast;
    }

    if river_connected {
        return ColumnFillProfile::River(classify_river_stage(sample));
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
    if sample.river_source_potential > 0.52
        || sample.river_flow_potential < 0.24
        || (sample.mountain_mass > 0.40 && sample.macro_elevation > 0.44)
    {
        RiverStage::Headwaters
    } else if sample.river_flow_potential < 0.58 {
        RiverStage::Middle
    } else {
        RiverStage::Lower
    }
}

fn apply_hydrology(
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
    base_surface_y: f32,
    fill_profile: ColumnFillProfile,
    land_threshold: f32,
    local_concavity: f32,
) -> HydrologyRealization {
    let inland = if sample.landness >= land_threshold && matches!(fill_profile, ColumnFillProfile::River(_)) {
        carve_river_channel(
            seed,
            world_x,
            world_z,
            sample,
            base_surface_y,
            fill_profile,
            local_concavity,
        )
    } else {
        None
    };
    let mut surface_y = inland.map(|realization| realization.surface_y).unwrap_or(base_surface_y);
    if matches!(fill_profile, ColumnFillProfile::Coast) {
        surface_y = surface_y.max(SEA_LEVEL_Y as f32);
    }
    let water_top_y = inland
        .and_then(|realization| realization.water_top_y)
        .or_else(|| match fill_profile {
            ColumnFillProfile::DeepOcean | ColumnFillProfile::ShallowOcean { .. }
                if surface_y < SEA_LEVEL_Y as f32 =>
            {
                Some(SEA_LEVEL_Y as f32)
            }
            _ => None,
        });

    HydrologyRealization {
        surface_y,
        water_top_y,
    }
}

fn carve_river_channel(
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
    base_surface_y: f32,
    fill_profile: ColumnFillProfile,
    local_concavity: f32,
) -> Option<HydrologyRealization> {
    let river_strength = clamp01(
        sample.riverine_factor * 1.10 + sample.lake_potential * 0.28 + local_concavity * 0.45 - 0.44,
    );
    if river_strength <= 0.05 || (local_concavity < 0.05 && sample.river_flow_potential < 0.24) {
        return None;
    }

    let meander_primary = centered_fbm(
        seed,
        world_x,
        world_z,
        168.0,
        4,
        2.0,
        0.5,
        RIVER_CHANNEL_PRIMARY_SALT,
    );
    let meander_secondary = centered_fbm(
        seed,
        world_x,
        world_z,
        80.0,
        3,
        2.0,
        0.5,
        RIVER_CHANNEL_SECONDARY_SALT,
    ) * 0.34;
    let centerline_distance = (meander_primary * 0.72 + meander_secondary * 0.28).abs();

    let mut channel_width =
        lerp_f32(0.044, 0.144, river_strength) + sample.lake_potential * 0.05 + local_concavity * 0.04;
    if matches!(fill_profile, ColumnFillProfile::Coast) {
        channel_width += 0.05;
    }
    let floodplain_width = channel_width * (2.1 + local_concavity * 2.0) + 0.08;
    let floodplain_mask = clamp01(1.0 - centerline_distance / floodplain_width);
    if floodplain_mask <= 0.0 {
        return None;
    }

    let channel_mask = clamp01(1.0 - centerline_distance / channel_width);
    let floodplain_drop =
        (0.8 + river_strength * 2.8 + sample.wetness * 1.6 + local_concavity * 1.8) * floodplain_mask;
    let floodplain_y = base_surface_y - floodplain_drop;
    if channel_mask <= 0.08 {
        return Some(HydrologyRealization {
            surface_y: floodplain_y,
            water_top_y: None,
        });
    }

    let river_stage = match fill_profile {
        ColumnFillProfile::River(stage) => stage,
        _ => classify_river_stage(sample),
    };
    let (channel_depth_base, water_depth_base) = match river_stage {
        RiverStage::Headwaters => (2.2, 0.9),
        RiverStage::Middle => (3.4, 1.4),
        RiverStage::Lower => (2.8, 1.6),
    };
    let channel_depth = (channel_depth_base
        + river_strength * 2.6
        + sample.river_flow_potential * 1.2
        + sample.wetness * 0.8
        + local_concavity * 3.2)
        * channel_mask;
    let bed_y = (floodplain_y - channel_depth.max(1.4)).min(base_surface_y - 0.8);
    let bank_freeboard = (0.40 - river_strength * 0.18 - local_concavity * 0.12).clamp(0.08, 0.40);
    let water_surface_y = floodplain_y - bank_freeboard;
    let min_water_depth = water_depth_base
        + river_strength * 0.7
        + local_concavity * 0.8
        + if river_strength > 0.64 || sample.lake_potential > 0.70 {
            0.8
        } else {
            0.0
        };
    let water_top_y = water_surface_y.max(bed_y + min_water_depth);

    Some(HydrologyRealization {
        surface_y: bed_y,
        water_top_y: (water_top_y > bed_y).then_some(water_top_y),
    })
}

pub(super) fn block_for_world_y(
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
            column.water_top_y.is_some_and(|water_top_y| water_top_y > column.surface_y),
            column.surface_y,
            palette,
        );
    }

    if column
        .water_top_y
        .is_some_and(|water_top_y| world_y > column.surface_y && world_y <= water_top_y)
    {
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
    submerged_surface: bool,
    surface_y: i32,
    palette: GenerationPalette,
) -> BlockId {
    let roll = hash01_3d(seed, world_x, world_y, world_z, MATERIAL_BLEND_SALT);
    let grass_topped_land_surface =
        is_surface && surface_y >= SEA_LEVEL_Y && !submerged_surface;

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
            if grass_topped_land_surface {
                palette.grass
            } else {
                palette.dirt
            }
        }
    }
}
