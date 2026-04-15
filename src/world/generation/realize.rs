use super::context::{
    ColumnAtlasSample, ColumnFillProfile, ColumnRealization, GenerationPalette, RiverStage,
};
use super::noise::{
    MATERIAL_BLEND_SALT, STONE_DEPTH_SALT, centered_fbm, clamp01, hash01_2d, lerp_f32,
    ridge_signal_fbm,
};
use super::sampler::{
    generate_chunk_atlas_fields, generate_chunk_atlas_structure, generate_chunk_meso_guides,
};
use super::surface::{PreparedStructureGuide, build_chunk_surface_field};
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
const MATERIAL_BOUNDARY_PRIMARY_SALT: u64 = MATERIAL_BLEND_SALT.wrapping_add(0x4200);
const MATERIAL_BOUNDARY_SECONDARY_SALT: u64 = MATERIAL_BLEND_SALT.wrapping_add(0x4201);
const MATERIAL_BOUNDARY_WARP_X_SALT: u64 = MATERIAL_BLEND_SALT.wrapping_add(0x4206);
const MATERIAL_BOUNDARY_WARP_Z_SALT: u64 = MATERIAL_BLEND_SALT.wrapping_add(0x4207);
const MATERIAL_BOUNDARY_DETAIL_SALT: u64 = MATERIAL_BLEND_SALT.wrapping_add(0x4208);
const SEDIMENT_ZONE_PRIMARY_SALT: u64 = MATERIAL_BLEND_SALT.wrapping_add(0x4202);
const SEDIMENT_ZONE_SECONDARY_SALT: u64 = MATERIAL_BLEND_SALT.wrapping_add(0x4203);
const GRAVEL_ZONE_PRIMARY_SALT: u64 = MATERIAL_BLEND_SALT.wrapping_add(0x4204);
const GRAVEL_ZONE_SECONDARY_SALT: u64 = MATERIAL_BLEND_SALT.wrapping_add(0x4205);
const POLAR_FROZEN_THRESHOLD: f32 = 0.52;
const ALPINE_FROZEN_THRESHOLD: f32 = 0.72;
const ALPINE_FROZEN_MAX_TEMPERATURE: f32 = 0.34;

#[derive(Debug, Clone, Copy)]
struct HydrologyRealization {
    surface_y: f32,
    water_top_y: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ColumnBlocks {
    pub surface_block: BlockId,
    pub fill_block: BlockId,
}

pub fn generate_chunk(coord: ChunkCoord, meta: &WorldMeta, registry: &BlockRegistry) -> ChunkData {
    let palette = GenerationPalette::from_registry(registry);
    let atlas_fields = generate_chunk_atlas_fields(coord, meta);
    let atlas_structure = generate_chunk_atlas_structure(coord, meta);
    let atlas_meso = generate_chunk_meso_guides(coord, meta, &atlas_fields, &atlas_structure);
    let surface_field =
        build_chunk_surface_field(coord, meta, &atlas_fields, &atlas_structure, &atlas_meso);
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
            let structure = surface_column.structure;
            let fill_profile = classify_fill_profile(
                meta.seed,
                world_x,
                world_z,
                atlas_sample,
                base_surface_y.round() as i32,
                land_threshold,
                profile,
                structure,
            );
            let hydrology = apply_hydrology(
                meta.seed,
                world_x,
                world_z,
                atlas_sample,
                base_surface_y,
                fill_profile,
                land_threshold,
                surface_column.local_concavity,
                structure,
            );
            let surface_y = hydrology.surface_y.round() as i32;
            let water_top_y = hydrology
                .water_top_y
                .map(|water_top_y| water_top_y.ceil() as i32)
                .filter(|water_top_y| *water_top_y > surface_y);
            let stone_ceiling_y =
                compute_stone_ceiling(meta.seed, world_x, world_z, surface_y);
            let realization = ColumnRealization {
                surface_y,
                stone_ceiling_y,
                water_top_y,
                fill_profile,
            };
            let blocks = resolve_column_blocks(
                meta.seed,
                world_x,
                world_z,
                atlas_sample,
                realization,
                palette,
            );

            for local_y in 0..CHUNK_EDGE_I32 as u8 {
                let local = LocalBlockCoord::new(local_x, local_y, local_z).unwrap();
                let world_y = chunk_local_to_world(coord, local).1;
                let block = block_for_world_y(world_y, realization, blocks, palette);
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
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
    surface_y: i32,
    land_threshold: f32,
    profile: super::profile::TerrainProfile,
    structure: PreparedStructureGuide,
) -> ColumnFillProfile {
    let boundary_offset = material_boundary_offset(
        seed,
        world_x,
        world_z,
        sample,
        land_threshold,
    );
    let warped_landness = clamp01(sample.landness + boundary_offset);
    let warped_coast_factor = clamp01(
        sample.coast_factor
            + boundary_offset * 1.6
            + (0.16 - sample.ocean_distance).max(0.0) * 1.4,
    );
    let is_land = warped_landness >= land_threshold;
    let river_connected = (sample.riverine_factor > 0.54 && sample.river_flow_potential > 0.14)
        || sample.river_flow_potential > 0.42
        || sample.lake_potential > 0.66
        || (structure.channel_weight > 0.24 && structure.channel_core > 0.04)
        || structure.channel_weight > 0.58;
    let emergent_shore = surface_y >= SEA_LEVEL_Y + 2
        && sample.ocean_distance < 0.18
        && sample.landness <= land_threshold + 0.04
        && sample.mountain_mass < 0.82;
    let coast_like = matches!(profile, super::profile::TerrainProfile::Coast)
        || emergent_shore
        || (warped_coast_factor > 0.38
            && surface_y <= SEA_LEVEL_Y + 14
            && sample.mountain_mass < 0.72);

    if coast_like {
        return ColumnFillProfile::Coast;
    }

    if !is_land {
        return match profile {
            super::profile::TerrainProfile::DeepOcean => ColumnFillProfile::DeepOcean,
            _ => ColumnFillProfile::ShallowOcean { river_connected },
        };
    }

    if river_connected {
        return ColumnFillProfile::River(classify_river_stage(sample, structure));
    }

    if sample.temperature > 0.56
        && sample.aridity > 0.60
        && sample.humidity < 0.42
        && sample.wetness < 0.38
    {
        return ColumnFillProfile::Desert;
    }

    if resolves_to_frozen_surface(sample) {
        return ColumnFillProfile::Frozen;
    }

    ColumnFillProfile::SoilWithGrassTop
}

fn resolves_to_frozen_surface(sample: ColumnAtlasSample) -> bool {
    if sample.polar_factor > POLAR_FROZEN_THRESHOLD {
        return true;
    }

    // `alpine_factor` is driven primarily by elevation and mountain mass, so it is a
    // mountain-ness signal rather than a guaranteed snow signal. Keep warm alpine ridges
    // grassy unless the thermal field is also cold enough to support snow cover.
    sample.alpine_factor > ALPINE_FROZEN_THRESHOLD
        && sample.temperature < ALPINE_FROZEN_MAX_TEMPERATURE
}

fn material_boundary_offset(
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
    land_threshold: f32,
) -> f32 {
    let warp_x = (centered_fbm(
        seed,
        world_x,
        world_z,
        168.0,
        3,
        2.0,
        0.5,
        MATERIAL_BOUNDARY_WARP_X_SALT,
    ) * 34.0)
        .round() as i32;
    let warp_z = (centered_fbm(
        seed,
        world_x + 941,
        world_z - 677,
        156.0,
        3,
        2.0,
        0.5,
        MATERIAL_BOUNDARY_WARP_Z_SALT,
    ) * 34.0)
        .round() as i32;
    let warped_x = world_x + warp_x;
    let warped_z = world_z + warp_z;
    let broad = centered_fbm(
        seed,
        world_x,
        world_z,
        320.0,
        3,
        2.0,
        0.5,
        MATERIAL_BOUNDARY_PRIMARY_SALT,
    );
    let meander = centered_fbm(
        seed,
        warped_x,
        warped_z,
        86.0,
        4,
        2.0,
        0.52,
        MATERIAL_BOUNDARY_SECONDARY_SALT,
    );
    let detail = ridge_signal_fbm(
        seed,
        warped_x,
        warped_z,
        40.0,
        3,
        2.0,
        0.58,
        MATERIAL_BOUNDARY_DETAIL_SALT,
    ) * 2.0
        - 1.0;
    let near_land_boundary =
        1.0 - ((sample.landness - land_threshold).abs() / 0.20).clamp(0.0, 1.0);
    let near_coast_boundary = 1.0 - ((sample.coast_factor - 0.38).abs() / 0.24).clamp(0.0, 1.0);
    let near_river_boundary =
        1.0 - ((sample.riverine_factor - 0.42).abs() / 0.30).clamp(0.0, 1.0);
    let edge_focus = near_land_boundary
        .max(near_coast_boundary)
        .max(near_river_boundary);

    broad * 0.020 + (meander * 0.66 + detail * 0.34) * (0.018 + edge_focus * 0.060)
}

fn classify_river_stage(sample: ColumnAtlasSample, structure: PreparedStructureGuide) -> RiverStage {
    let downstream_factor = downstream_progress_factor(structure);

    if structure.confluence_weight > 0.52 && structure.channel_order >= 2 {
        if downstream_factor > 0.42 || sample.river_flow_potential > 0.38 {
            return RiverStage::Lower;
        }
        return RiverStage::Middle;
    }

    if structure.channel_order >= 2 {
        if downstream_factor > 0.62
            || sample.river_flow_potential > 0.56
            || sample.lake_potential > 0.42
        {
            return RiverStage::Lower;
        }
        if downstream_factor > 0.18 {
            return RiverStage::Middle;
        }
    }

    if sample.river_source_potential > 0.52
        || downstream_factor < 0.18
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
    structure: PreparedStructureGuide,
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
            structure,
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
    structure: PreparedStructureGuide,
) -> Option<HydrologyRealization> {
    let downstream_factor = downstream_progress_factor(structure);
    let confluence_factor = structure.confluence_weight;
    let river_strength = clamp01(
        sample.riverine_factor * 0.72
            + structure.channel_weight * 0.96
            + structure.channel_core * 0.74
            + confluence_factor * 0.52
            + sample.lake_potential * 0.24
            + local_concavity * 0.24
            - 0.34,
    );
    if river_strength <= 0.05
        || (structure.channel_weight <= 0.02
            && local_concavity < 0.05
            && sample.river_flow_potential < 0.24)
    {
        return None;
    }

    let scalar_meander_primary = centered_fbm(
        seed,
        world_x,
        world_z,
        168.0,
        4,
        2.0,
        0.5,
        RIVER_CHANNEL_PRIMARY_SALT,
    );
    let scalar_meander_secondary = centered_fbm(
        seed,
        world_x,
        world_z,
        80.0,
        3,
        2.0,
        0.5,
        RIVER_CHANNEL_SECONDARY_SALT,
    ) * 0.34;
    let scalar_centerline_distance =
        (scalar_meander_primary * 0.72 + scalar_meander_secondary * 0.28).abs();
    let structure_centerline_distance = if structure.channel_weight > 0.0 {
        let meander_offset = structure_meander_offset_cells(seed, structure, sample, local_concavity);
        (structure.channel_signed_distance_cells - meander_offset).abs()
    } else {
        f32::INFINITY
    };

    let mut channel_width =
        lerp_f32(0.044, 0.144, river_strength)
            + sample.lake_potential * 0.05
            + local_concavity * 0.04;
    if structure.channel_weight > 0.0 {
        channel_width = channel_width.max(0.026 + structure.channel_bankfull_hint * 0.014);
    }
    if matches!(fill_profile, ColumnFillProfile::Coast) {
        channel_width += 0.05;
    }
    channel_width += confluence_factor * (0.022 + structure.confluence_order as f32 * 0.010);
    let floodplain_width = channel_width * (2.1 + local_concavity * 2.0) + 0.08;
    let scalar_floodplain_mask = clamp01(1.0 - scalar_centerline_distance / floodplain_width);
    let structure_floodplain_mask = clamp01(1.0 - structure_centerline_distance / floodplain_width);
    let floodplain_mask = if structure.channel_weight > 0.0 {
        structure_floodplain_mask.max(scalar_floodplain_mask * 0.22)
    } else {
        scalar_floodplain_mask
    };
    if floodplain_mask <= 0.0 {
        return None;
    }

    let scalar_channel_mask = clamp01(1.0 - scalar_centerline_distance / channel_width);
    let structure_channel_mask = clamp01(1.0 - structure_centerline_distance / channel_width);
    let channel_mask = if structure.channel_weight > 0.0 {
        let confluence_core = confluence_factor * (0.26 + structure.confluence_order as f32 * 0.06);
        structure
            .channel_core
            .max(structure_channel_mask.max(scalar_channel_mask * 0.12))
            .max(confluence_core.clamp(0.0, 1.0))
    } else {
        scalar_channel_mask
    };
    let floodplain_drop =
        (0.8
            + river_strength * 2.8
            + sample.wetness * 1.6
            + local_concavity * 1.8
            + confluence_factor * (0.9 + structure.confluence_order as f32 * 0.5))
            * floodplain_mask;
    let downstream_grade_drop = downstream_water_grade(structure, downstream_factor);
    let graded_surface_y = base_surface_y - downstream_grade_drop;
    let floodplain_y = graded_surface_y - floodplain_drop;
    let dry_channel_threshold = (0.08 - confluence_factor * 0.035).clamp(0.03, 0.08);
    if channel_mask <= dry_channel_threshold {
        return Some(HydrologyRealization {
            surface_y: floodplain_y,
            water_top_y: None,
        });
    }

    let river_stage = match fill_profile {
        ColumnFillProfile::River(stage) => stage,
        _ => classify_river_stage(sample, structure),
    };
    let (channel_depth_base, water_depth_base) = match river_stage {
        RiverStage::Headwaters => (2.2, 0.9),
        RiverStage::Middle => (3.4, 1.4),
        RiverStage::Lower => (2.8, 1.6),
    };
    let channel_depth = (channel_depth_base
        + river_strength * 2.6
        + structure.channel_weight * 2.2
        + confluence_factor * (1.4 + structure.confluence_order as f32 * 0.8)
        + sample.river_flow_potential * 1.2
        + sample.wetness * 0.8
        + local_concavity * 3.2)
        * channel_mask;
    let branch_waterline_inset = match river_stage {
        RiverStage::Headwaters => 1.18,
        RiverStage::Middle => 1.32,
        RiverStage::Lower => 1.46,
    };
    let water_surface_y = graded_surface_y
        - (branch_waterline_inset
            + river_strength * 0.16
            + local_concavity * 0.04
            - confluence_factor * 0.08)
            .clamp(0.72, 1.92);
    let min_water_depth = water_depth_base
        + river_strength * 0.7
        + structure.channel_core * 0.9
        + confluence_factor * (0.55 + structure.confluence_order as f32 * 0.30)
        + downstream_factor * 0.5
        + local_concavity * 0.8
        + if river_strength > 0.64 || sample.lake_potential > 0.70 {
            0.8
        } else {
            0.0
        };
    let min_water_depth = min_water_depth.max(1.05);
    let bed_y = (floodplain_y - channel_depth.max(1.4)).min(water_surface_y - min_water_depth);
    let water_top_y = water_surface_y.max(bed_y + min_water_depth);

    Some(HydrologyRealization {
        surface_y: bed_y,
        water_top_y: (water_top_y > bed_y).then_some(water_top_y),
    })
}

fn downstream_progress_factor(structure: PreparedStructureGuide) -> f32 {
    let normalization = match structure.channel_order {
        0 => return 0.0,
        1 => 10.0,
        _ => 18.0,
    };
    clamp01(structure.along_channel_cells / normalization)
}

fn downstream_water_grade(structure: PreparedStructureGuide, downstream_factor: f32) -> f32 {
    let grade_scale = match structure.channel_order {
        0 => 0.0,
        1 => 1.4,
        _ => 2.8,
    };
    downstream_factor * grade_scale
}

fn structure_meander_offset_cells(
    seed: u64,
    structure: PreparedStructureGuide,
    sample: ColumnAtlasSample,
    local_concavity: f32,
) -> f32 {
    if structure.channel_order == 0 {
        return 0.0;
    }

    let along_blocks = (structure.along_channel_cells * 512.0).round() as i32;
    let branch_seed = seed ^ u64::from(structure.channel_id);
    let primary = centered_fbm(
        branch_seed,
        along_blocks,
        structure.channel_order as i32 * 101,
        420.0,
        4,
        2.0,
        0.52,
        RIVER_CHANNEL_PRIMARY_SALT,
    );
    let secondary = centered_fbm(
        branch_seed,
        along_blocks,
        structure.channel_order as i32 * 211,
        180.0,
        3,
        2.0,
        0.52,
        RIVER_CHANNEL_SECONDARY_SALT,
    );
    let amplitude = (0.010
        + structure.channel_bankfull_hint * 0.010
        + structure.channel_order as f32 * 0.006
        + sample.wetness * 0.010
        + local_concavity * 0.006)
        .clamp(0.012, 0.090);

    (primary * 0.72 + secondary * 0.28) * amplitude
}

pub(super) fn block_for_world_y(
    world_y: i32,
    column: ColumnRealization,
    blocks: ColumnBlocks,
    palette: GenerationPalette,
) -> BlockId {
    if world_y < WORLD_FLOOR_Y {
        return BlockId::AIR;
    }

    if world_y <= column.stone_ceiling_y {
        return palette.stone;
    }

    if world_y <= column.surface_y {
        if world_y == column.surface_y {
            return blocks.surface_block;
        }
        return blocks.fill_block;
    }

    if column
        .water_top_y
        .is_some_and(|water_top_y| world_y > column.surface_y && world_y <= water_top_y)
    {
        return palette.water;
    }

    BlockId::AIR
}

fn resolve_column_blocks(
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
    column: ColumnRealization,
    palette: GenerationPalette,
) -> ColumnBlocks {
    let submerged_surface = column
        .water_top_y
        .is_some_and(|water_top_y| water_top_y > column.surface_y);

    match column.fill_profile {
        ColumnFillProfile::DeepOcean => ColumnBlocks {
            surface_block: palette.mud,
            fill_block: palette.mud,
        },
        ColumnFillProfile::ShallowOcean { river_connected } => {
            let sediment =
                select_shallow_ocean_sediment(seed, world_x, world_z, sample, river_connected, palette);
            ColumnBlocks {
                surface_block: sediment,
                fill_block: sediment,
            }
        }
        ColumnFillProfile::River(stage) => {
            let sediment = select_river_bed_material(seed, world_x, world_z, sample, stage, palette);
            ColumnBlocks {
                surface_block: sediment,
                fill_block: sediment,
            }
        }
        ColumnFillProfile::Coast | ColumnFillProfile::Desert => ColumnBlocks {
            surface_block: palette.sand,
            fill_block: palette.sand,
        },
        ColumnFillProfile::Frozen => ColumnBlocks {
            surface_block: palette.snow,
            fill_block: palette.snow,
        },
        ColumnFillProfile::SoilWithGrassTop => ColumnBlocks {
            surface_block: if column.surface_y >= SEA_LEVEL_Y && !submerged_surface {
                palette.grass
            } else {
                palette.dirt
            },
            fill_block: palette.dirt,
        },
    }
}

fn select_shallow_ocean_sediment(
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
    river_connected: bool,
    palette: GenerationPalette,
) -> BlockId {
    let sediment = sediment_zone_signal(seed, world_x, world_z);
    let gravel = gravel_zone_signal(seed, world_x, world_z);
    let mud_bias = sample.wetness * 0.28
        + sample.lake_potential * 0.20
        + if river_connected { 0.10 } else { 0.0 };
    let sand_score = 0.56 + sediment * 0.24 - mud_bias * 0.24;
    let mud_score = 0.40 - sediment * 0.20 + mud_bias * 0.30;
    let gravel_threshold = if river_connected { 0.90 } else { 0.95 };

    if gravel > gravel_threshold {
        palette.gravel
    } else if sand_score >= mud_score {
        palette.sand
    } else {
        palette.mud
    }
}

fn select_river_bed_material(
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
    stage: RiverStage,
    palette: GenerationPalette,
) -> BlockId {
    let sediment = sediment_zone_signal(seed, world_x, world_z);
    let gravel = gravel_zone_signal(seed, world_x, world_z);
    let lower_silt_bias = sample.wetness * 0.20 + sample.lake_potential * 0.20;

    match stage {
        RiverStage::Headwaters => {
            if gravel > 0.44 {
                palette.gravel
            } else {
                palette.sand
            }
        }
        RiverStage::Middle => {
            if gravel > 0.70 {
                palette.gravel
            } else if sediment > -0.12 {
                palette.sand
            } else {
                palette.mud
            }
        }
        RiverStage::Lower => {
            if sediment + lower_silt_bias > 0.10 {
                palette.sand
            } else {
                palette.mud
            }
        }
    }
}

fn sediment_zone_signal(seed: u64, world_x: i32, world_z: i32) -> f32 {
    let warped = warped_material_coords(seed, world_x, world_z);
    let broad = centered_fbm(
        seed,
        warped.0,
        warped.1,
        224.0,
        4,
        2.0,
        0.5,
        SEDIMENT_ZONE_PRIMARY_SALT,
    );
    let medium = centered_fbm(
        seed,
        warped.0,
        warped.1,
        88.0,
        3,
        2.0,
        0.5,
        SEDIMENT_ZONE_SECONDARY_SALT,
    );
    broad * 0.72 + medium * 0.28
}

fn gravel_zone_signal(seed: u64, world_x: i32, world_z: i32) -> f32 {
    let warped = warped_material_coords(seed ^ 0x9137, world_x, world_z);
    let ridged = ridge_signal_fbm(
        seed,
        warped.0,
        warped.1,
        136.0,
        3,
        2.0,
        0.55,
        GRAVEL_ZONE_PRIMARY_SALT,
    );
    let patch = clamp01(
        centered_fbm(
            seed,
            warped.0,
            warped.1,
            56.0,
            2,
            2.0,
            0.5,
            GRAVEL_ZONE_SECONDARY_SALT,
        ) * 0.5
            + 0.5,
    );
    clamp01(ridged * 0.72 + patch * 0.28)
}

fn warped_material_coords(seed: u64, world_x: i32, world_z: i32) -> (i32, i32) {
    let warp_x = (centered_fbm(
        seed,
        world_x,
        world_z,
        144.0,
        3,
        2.0,
        0.5,
        MATERIAL_BOUNDARY_WARP_X_SALT,
    ) * 24.0)
        .round() as i32;
    let warp_z = (centered_fbm(
        seed,
        world_x - 773,
        world_z + 551,
        132.0,
        3,
        2.0,
        0.5,
        MATERIAL_BOUNDARY_WARP_Z_SALT,
    ) * 24.0)
        .round() as i32;
    (world_x + warp_x, world_z + warp_z)
}
