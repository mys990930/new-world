use std::f32::consts::TAU;

use crate::world::atlas::{AtlasCell, AtlasCoord, MesoGuideCell, MesoGuideMap};
use crate::world::{CHUNK_EDGE_I32, MESO_GUIDE_CELL_SIZE_IN_CHUNKS};

use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};
use super::super::{
    FeatureInstance, MesoFeatureKind, cell_center_jitter, ellipse_footprint, hash01, lerp_f32,
    smoothstep_range,
};

const STRENGTH_MIN_BLOCKS: f32 = 6.4;
const STRENGTH_MAX_BLOCKS: f32 = 17.6;
const LOBE_HASH_K1: u64 = 0x9E37_79B9_7F4A_7C15;
const LOBE_HASH_K2: u64 = 0xC2B2_AE3D_27D4_EB4F;
const LOBE_HASH_K3: u64 = 0x1656_67B1_9E37_79F9;
const APPLY_AXIS_FALLBACK_SALT: u64 = 0xD811_B6D2_2200_0001;
const SOURCE_CENTER_X_SALT: u64 = 0xD811_B6D2_2200_0002;
const SOURCE_CENTER_Z_SALT: u64 = 0xD811_B6D2_2200_0003;
const SOURCE_AXIS_JITTER_SALT: u64 = 0xD811_B6D2_2200_0004;
const SOURCE_CHAIN_SPACING_SALT: u64 = 0xD811_B6D2_2200_0005;
const SOURCE_MAJOR_RADIUS_SALT: u64 = 0xD811_B6D2_2200_0006;
const SOURCE_MINOR_RADIUS_SALT: u64 = 0xD811_B6D2_2200_0007;
const SOURCE_HEIGHT_SALT: u64 = 0xD811_B6D2_2200_0008;
const SOURCE_ALONG_JITTER_SALT: u64 = 0xD811_B6D2_2200_0009;
const SOURCE_SIDE_JITTER_SALT: u64 = 0xD811_B6D2_2200_000A;
const SOURCE_COUNT_SALT: u64 = 0xD811_B6D2_2200_000B;
const APPLY_SCAN_RADIUS_CELLS: i32 = 4;
const MAX_APPLY_SOURCES: usize = 81;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HillClusterApplySample {
    pub coverage: f32,
    pub shoulder_coverage: f32,
    pub lobe_height_blocks: f32,
}

#[derive(Debug, Clone, Copy)]
struct MacroLobeDescriptor {
    center_x: f32,
    center_z: f32,
    heading_x: f32,
    heading_z: f32,
    radius_x_blocks: f32,
    radius_z_blocks: f32,
    height_blocks: f32,
}

#[derive(Debug, Clone, Copy)]
struct GuideSource {
    coord: AtlasCoord,
    cell: MesoGuideCell,
    center_x: f32,
    center_z: f32,
    weight: f32,
}

impl Default for GuideSource {
    fn default() -> Self {
        Self {
            coord: AtlasCoord::new(0, 0),
            cell: MesoGuideCell::default(),
            center_x: 0.0,
            center_z: 0.0,
            weight: 0.0,
        }
    }
}

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "hill_cluster",
    summary: "Deterministic multi-peak hill groups for inland multi-chunk relief.",
    placement_family: MesoPlacementFamily::InteriorLandform,
    hydrology_coupling: MesoHydrologyCoupling::AvoidPrimaryCorridor,
    terrain_effects: &[
        "Biases inland terrain prototypes without taking over the owning region identity.",
        "Rasterizes as several nearby hilltops under one shared cluster envelope instead of one broad swell.",
        "Should avoid displacing major river corridors and instead sit beside or above them.",
    ],
    ecology_notes: &[
        "Later ecology can use this feature to break uniform cover into readable local habitat patches.",
        "This candidate is in the near-term planning set, so archetype allowances and deformation operators should be locked first.",
    ],
};

pub(in crate::world::atlas::meso) fn build_instance(
    seed: u64,
    cell_coord: AtlasCoord,
    sample_point: (f32, f32),
    sample: AtlasCell,
    heading: (f32, f32),
) -> FeatureInstance {
    let jitter = cell_center_jitter(seed, cell_coord);
    let center_x = sample_point.0 * crate::world::MESO_GUIDE_CELLS_PER_ATLAS_CELL as f32 + jitter.0;
    let center_z = sample_point.1 * crate::world::MESO_GUIDE_CELLS_PER_ATLAS_CELL as f32 + jitter.1;
    let strength_scale =
        (0.92 + sample.macro_elevation * 0.30 + sample.ruggedness * 0.18).clamp(0.92, 1.34);

    FeatureInstance {
        kind: MesoFeatureKind::HillCluster,
        center_x,
        center_z,
        heading_x: heading.0,
        heading_z: heading.1,
        radius_x_cells: lerp_f32(
            2.22,
            3.84,
            hash01(
                seed,
                cell_coord.x as i64,
                cell_coord.z as i64,
                super::super::MESO_FEATURE_RADIUS_X_SALT,
            ),
        ),
        radius_z_cells: lerp_f32(
            2.18,
            3.58,
            hash01(
                seed,
                cell_coord.x as i64,
                cell_coord.z as i64,
                super::super::MESO_FEATURE_RADIUS_Z_SALT,
            ),
        ),
        strength_blocks: lerp_f32(
            STRENGTH_MIN_BLOCKS,
            STRENGTH_MAX_BLOCKS,
            hash01(
                seed,
                cell_coord.x as i64,
                cell_coord.z as i64,
                super::super::MESO_FEATURE_STRENGTH_SALT,
            ),
        ) * strength_scale,
        spacing_cells: 1.0,
    }
}

pub(in crate::world::atlas::meso) fn rasterize(
    instance: FeatureInstance,
    cell: &mut MesoGuideCell,
    along: f32,
    across: f32,
) {
    let envelope = ellipse_footprint(
        along,
        across,
        instance.radius_x_cells * 1.08,
        instance.radius_z_cells * 1.10,
    );
    if envelope <= 0.0 {
        return;
    }

    let masses = cluster_mass_footprints(instance, along, across);
    let strongest_mass = masses
        .into_iter()
        .fold(0.0_f32, f32::max);
    let weighted_mass_sum =
        masses[0] * 0.82 + masses[1] * 1.00 + masses[2] * 0.90 + masses[3] * 0.74 + masses[4] * 0.66;
    let mass_blend = (weighted_mass_sum / 3.30).clamp(0.0, 1.0);
    let saddle_fill = ellipse_footprint(
        along,
        across,
        instance.radius_x_cells * 0.74,
        instance.radius_z_cells * 0.46,
    ) * 0.22;
    let shoulder_fill = smoothstep_range(1.00, 0.18, envelope) * 0.24;
    let hilliness = (
        envelope * 0.24
            + strongest_mass * 0.24
            + mass_blend * 0.36
            + saddle_fill * 0.16
            + shoulder_fill * 0.10
    )
        .clamp(0.0, 1.0);
    let height_scale = (
        envelope * 0.18
            + strongest_mass * 0.34
            + mass_blend * 0.34
            + saddle_fill * 0.08
            + shoulder_fill * 0.10
    )
        .clamp(0.0, 1.28);

    cell.hilliness = (cell.hilliness + hilliness * 0.88).clamp(0.0, 1.0);
    cell.hill_height = cell.hill_height.max(instance.strength_blocks * height_scale);
}

pub(crate) fn sample_apply_signal(
    guides: &MesoGuideMap,
    world_x: i32,
    world_z: i32,
) -> HillClusterApplySample {
    let meso_span_blocks = (CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32).max(1);
    let base_cell_x = world_x.div_euclid(meso_span_blocks);
    let base_cell_z = world_z.div_euclid(meso_span_blocks);
    let sample_x = world_x as f32 + 0.5;
    let sample_z = world_z as f32 + 0.5;
    let mut sources = [GuideSource::default(); MAX_APPLY_SOURCES];
    let mut source_count = 0_usize;
    let mut strongest = 0.0_f32;
    let mut second = 0.0_f32;
    let mut third = 0.0_f32;
    let mut coverage = 0.0_f32;
    let mut shoulder_coverage = 0.0_f32;

    for cell_z in (base_cell_z - APPLY_SCAN_RADIUS_CELLS)..=(base_cell_z + APPLY_SCAN_RADIUS_CELLS)
    {
        for cell_x in (base_cell_x - APPLY_SCAN_RADIUS_CELLS)
            ..=(base_cell_x + APPLY_SCAN_RADIUS_CELLS)
        {
            let coord = AtlasCoord::new(cell_x, cell_z);
            let Some(cell) = guides.cells().get(coord).copied() else {
                continue;
            };
            let Some(source) = guide_source(coord, cell, meso_span_blocks as f32) else {
                continue;
            };
            sources[source_count] = source;
            source_count += 1;
        }
    }

    if source_count == 0 {
        return HillClusterApplySample::default();
    }

    let cluster_heading = dominant_apply_axis(&sources, source_count, base_cell_x, base_cell_z);

    for source in sources.iter().take(source_count) {
        let (heading_x, heading_z) = source_chain_heading(*source, cluster_heading);
        let normal_x = -heading_z;
        let normal_z = heading_x;
        let major_scale =
            (0.88 + source.cell.hilliness * 0.18 + (source.cell.hill_height / 18.0).clamp(0.0, 0.28))
                .clamp(0.88, 1.34);
        let minor_scale =
            (0.78 + source.cell.hilliness * 0.16 + (source.cell.hill_height / 22.0).clamp(0.0, 0.16))
                .clamp(0.78, 1.12);
        let base_major = lerp_f32(
            39.0,
            62.0,
            lobe_hash01(source.coord, source.cell, SOURCE_MAJOR_RADIUS_SALT),
        ) * major_scale;
        let base_minor = lerp_f32(
            28.0,
            44.0,
            lobe_hash01(source.coord, source.cell, SOURCE_MINOR_RADIUS_SALT),
        ) * minor_scale;
        let chain_spacing = lerp_f32(
            15.0,
            24.0,
            lobe_hash01(source.coord, source.cell, SOURCE_CHAIN_SPACING_SALT),
        ) * (0.92 + source.cell.hilliness * 0.24);
        let lobe_count = if lobe_hash01(source.coord, source.cell, SOURCE_COUNT_SALT) >= 0.64
            || source.cell.hilliness >= 0.80
            || source.cell.hill_height >= 11.5
        {
            4
        } else {
            3
        };
        let chain_span = chain_spacing * (lobe_count - 1) as f32;
        let delta_x = sample_x - source.center_x;
        let delta_z = sample_z - source.center_z;
        let source_along = delta_x * heading_x + delta_z * heading_z;
        let source_across = delta_x * normal_x + delta_z * normal_z;
        let shoulder = ellipse_footprint(
            source_along,
            source_across,
            base_major + chain_span * 1.02,
            base_minor * 1.92 + 9.0,
        );
        shoulder_coverage = shoulder_coverage.max(
            (shoulder * (0.22 + source.cell.hilliness * 0.34)).clamp(0.0, 0.88),
        );

        for lobe_index in 0..lobe_count {
            let progress = if lobe_count <= 1 {
                0.5
            } else {
                lobe_index as f32 / (lobe_count - 1) as f32
            };
            let center_bias = 1.0 - (progress * 2.0 - 1.0).abs();
            let lobe = macro_lobe_descriptor(
                *source,
                heading_x,
                heading_z,
                normal_x,
                normal_z,
                base_major,
                base_minor,
                chain_span,
                chain_spacing,
                lobe_index,
                progress,
                center_bias,
            );
            let lobe_delta_x = sample_x - lobe.center_x;
            let lobe_delta_z = sample_z - lobe.center_z;
            let along = lobe_delta_x * lobe.heading_x + lobe_delta_z * lobe.heading_z;
            let across = lobe_delta_x * -lobe.heading_z + lobe_delta_z * lobe.heading_x;
            let footprint = ellipse_footprint(
                along,
                across,
                lobe.radius_x_blocks.max(f32::EPSILON),
                lobe.radius_z_blocks.max(f32::EPSILON),
            );
            if footprint <= 0.0 {
                continue;
            }

            let contribution = lobe.height_blocks * footprint;
            insert_top3(contribution, &mut strongest, &mut second, &mut third);
            let core_mask = (footprint * (0.38 + source.cell.hilliness * 0.46)).clamp(0.0, 1.0);
            let shoulder_mask = (footprint * 0.20 + shoulder * (0.14 + source.cell.hilliness * 0.10))
                .clamp(0.0, 1.0);
            coverage = coverage.max(core_mask);
            shoulder_coverage = shoulder_coverage.max(shoulder_mask);
        }
    }

    if strongest <= f32::EPSILON {
        return HillClusterApplySample::default();
    }

    HillClusterApplySample {
        coverage: coverage.clamp(0.0, 1.0),
        shoulder_coverage: shoulder_coverage.max(coverage).clamp(0.0, 1.0),
        lobe_height_blocks: strongest + second * 0.66 + third * 0.30,
    }
}

fn cluster_mass_footprints(instance: FeatureInstance, along: f32, across: f32) -> [f32; 5] {
    [
        ellipse_footprint(
            along + instance.radius_x_cells * 0.76,
            across - instance.radius_z_cells * 0.24,
            instance.radius_x_cells * 0.34,
            instance.radius_z_cells * 0.46,
        ),
        ellipse_footprint(
            along + instance.radius_x_cells * 0.10,
            across + instance.radius_z_cells * 0.36,
            instance.radius_x_cells * 0.42,
            instance.radius_z_cells * 0.60,
        ),
        ellipse_footprint(
            along - instance.radius_x_cells * 0.52,
            across - instance.radius_z_cells * 0.34,
            instance.radius_x_cells * 0.44,
            instance.radius_z_cells * 0.58,
        ),
        ellipse_footprint(
            along - instance.radius_x_cells * 0.88,
            across + instance.radius_z_cells * 0.16,
            instance.radius_x_cells * 0.28,
            instance.radius_z_cells * 0.42,
        ),
        ellipse_footprint(
            along,
            across,
            instance.radius_x_cells * 0.54,
            instance.radius_z_cells * 0.78,
        ),
    ]
}

fn guide_source(coord: AtlasCoord, cell: MesoGuideCell, meso_span_blocks: f32) -> Option<GuideSource> {
    if cell.hilliness < 0.18 || cell.hill_height < 2.2 {
        return None;
    }

    let center_x = coord.x as f32 * meso_span_blocks
        + meso_span_blocks * 0.5
        + lerp_f32(
            -meso_span_blocks * 0.16,
            meso_span_blocks * 0.16,
            lobe_hash01(coord, cell, SOURCE_CENTER_X_SALT),
        );
    let center_z = coord.z as f32 * meso_span_blocks
        + meso_span_blocks * 0.5
        + lerp_f32(
            -meso_span_blocks * 0.16,
            meso_span_blocks * 0.16,
            lobe_hash01(coord, cell, SOURCE_CENTER_Z_SALT),
        );
    let weight = (0.24 + cell.hilliness * 0.56 + (cell.hill_height / 16.0).clamp(0.0, 0.44))
        .clamp(0.24, 1.32);

    Some(GuideSource {
        coord,
        cell,
        center_x,
        center_z,
        weight,
    })
}

fn dominant_apply_axis(
    sources: &[GuideSource; MAX_APPLY_SOURCES],
    source_count: usize,
    base_cell_x: i32,
    base_cell_z: i32,
) -> (f32, f32) {
    let mut total_weight = 0.0_f32;
    let mut mean_x = 0.0_f32;
    let mut mean_z = 0.0_f32;

    for source in sources.iter().take(source_count) {
        total_weight += source.weight;
        mean_x += source.center_x * source.weight;
        mean_z += source.center_z * source.weight;
    }

    if total_weight <= f32::EPSILON {
        return fallback_axis(base_cell_x, base_cell_z);
    }

    mean_x /= total_weight;
    mean_z /= total_weight;

    let mut xx = 0.0_f32;
    let mut zz = 0.0_f32;
    let mut xz = 0.0_f32;

    for source in sources.iter().take(source_count) {
        let delta_x = source.center_x - mean_x;
        let delta_z = source.center_z - mean_z;
        xx += source.weight * delta_x * delta_x;
        zz += source.weight * delta_z * delta_z;
        xz += source.weight * delta_x * delta_z;
    }

    if (xx + zz) <= 24.0 {
        return fallback_axis(base_cell_x, base_cell_z);
    }

    let angle = 0.5 * (2.0 * xz).atan2(xx - zz);
    let heading_x = angle.cos();
    let heading_z = angle.sin();
    if !heading_x.is_finite() || !heading_z.is_finite() {
        return fallback_axis(base_cell_x, base_cell_z);
    }

    (heading_x, heading_z)
}

fn fallback_axis(base_cell_x: i32, base_cell_z: i32) -> (f32, f32) {
    let bits = splitmix64(
        APPLY_AXIS_FALLBACK_SALT
            ^ (base_cell_x as i64 as u64).wrapping_mul(LOBE_HASH_K1)
            ^ (base_cell_z as i64 as u64).wrapping_mul(LOBE_HASH_K2)
            ^ LOBE_HASH_K3,
    ) >> 11;
    let max = ((1_u64 << 53) - 1) as f64;
    let angle = (bits as f64 / max) as f32 * TAU;
    (angle.cos(), angle.sin())
}

fn source_chain_heading(source: GuideSource, cluster_heading: (f32, f32)) -> (f32, f32) {
    let jitter_angle = lerp_f32(
        -0.28,
        0.28,
        lobe_hash01(source.coord, source.cell, SOURCE_AXIS_JITTER_SALT),
    ) * (0.50 + source.cell.hilliness * 0.14);
    rotate_vector(cluster_heading, jitter_angle)
}

fn rotate_vector(vector: (f32, f32), angle: f32) -> (f32, f32) {
    let sin = angle.sin();
    let cos = angle.cos();
    (
        vector.0 * cos - vector.1 * sin,
        vector.0 * sin + vector.1 * cos,
    )
}

fn macro_lobe_descriptor(
    source: GuideSource,
    heading_x: f32,
    heading_z: f32,
    normal_x: f32,
    normal_z: f32,
    base_major: f32,
    base_minor: f32,
    chain_span: f32,
    chain_spacing: f32,
    lobe_index: usize,
    progress: f32,
    center_bias: f32,
) -> MacroLobeDescriptor {
    let along_jitter = lerp_f32(
        -chain_spacing * 0.20,
        chain_spacing * 0.20,
        indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_ALONG_JITTER_SALT),
    );
    let patterned_side = match lobe_index {
        0 => -0.34,
        1 => 0.10,
        2 => 0.28,
        _ => -0.18,
    };
    let side_jitter = lerp_f32(
        -base_minor * 0.42,
        base_minor * 0.42,
        indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_SIDE_JITTER_SALT),
    );
    let offset_along = (progress - 0.5) * chain_span + along_jitter;
    let offset_across = base_minor * patterned_side + side_jitter;
    let radius_x = base_major
        * (0.88 + center_bias * 0.22 + source.cell.hilliness * 0.08)
        * lerp_f32(
            0.98,
            1.14,
            indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_MAJOR_RADIUS_SALT),
        );
    let radius_z = base_minor
        * (1.00 + center_bias * 0.12 + source.cell.hilliness * 0.06)
        * lerp_f32(
            1.00,
            1.22,
            indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_MINOR_RADIUS_SALT),
        );
    let height_blocks = source.cell.hill_height
        * (1.04 + source.cell.hilliness * 0.30 + center_bias * 0.16)
        * lerp_f32(
            1.02,
            1.40,
            indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_HEIGHT_SALT),
        );

    MacroLobeDescriptor {
        center_x: source.center_x + heading_x * offset_along + normal_x * offset_across,
        center_z: source.center_z + heading_z * offset_along + normal_z * offset_across,
        heading_x,
        heading_z,
        radius_x_blocks: radius_x,
        radius_z_blocks: radius_z,
        height_blocks,
    }
}

fn insert_top3(value: f32, strongest: &mut f32, second: &mut f32, third: &mut f32) {
    if value > *strongest {
        *third = *second;
        *second = *strongest;
        *strongest = value;
    } else if value > *second {
        *third = *second;
        *second = value;
    } else if value > *third {
        *third = value;
    }
}

fn indexed_lobe_hash01(coord: AtlasCoord, cell: MesoGuideCell, lobe_index: usize, salt: u64) -> f32 {
    let hill_bits = ((cell.hilliness.to_bits() as u64) << 32) ^ cell.hill_height.to_bits() as u64;
    let bits = splitmix64(
        salt
            ^ hill_bits
            ^ (coord.x as i64 as u64).wrapping_mul(LOBE_HASH_K1)
            ^ (coord.z as i64 as u64).wrapping_mul(LOBE_HASH_K2)
            ^ (lobe_index as u64).wrapping_mul(LOBE_HASH_K3)
            ^ LOBE_HASH_K3,
    ) >> 11;
    let max = ((1_u64 << 53) - 1) as f64;
    (bits as f64 / max) as f32
}

#[cfg(test)]
fn source_weight_sum(sources: &[GuideSource; MAX_APPLY_SOURCES], source_count: usize) -> f32 {
    sources
        .iter()
        .take(source_count)
        .map(|source| source.weight)
        .sum::<f32>()
}

#[cfg(test)]
fn anisotropy_ratio(sources: &[GuideSource; MAX_APPLY_SOURCES], source_count: usize) -> f32 {
    let total_weight = source_weight_sum(sources, source_count);
    if total_weight <= f32::EPSILON {
        return 1.0;
    }

    let mean_x = sources
        .iter()
        .take(source_count)
        .map(|source| source.center_x * source.weight)
        .sum::<f32>()
        / total_weight;
    let mean_z = sources
        .iter()
        .take(source_count)
        .map(|source| source.center_z * source.weight)
        .sum::<f32>()
        / total_weight;
    let mut xx = 0.0_f32;
    let mut zz = 0.0_f32;

    for source in sources.iter().take(source_count) {
        let delta_x = source.center_x - mean_x;
        let delta_z = source.center_z - mean_z;
        xx += source.weight * delta_x * delta_x;
        zz += source.weight * delta_z * delta_z;
    }

    let major = xx.max(zz).sqrt();
    let minor = xx.min(zz).sqrt().max(1.0);
    major / minor
}

#[cfg(test)]
fn horizontal_alignment_ratio(heading: (f32, f32)) -> f32 {
    heading.0.abs() / heading.1.abs().max(0.001)
}

#[cfg(test)]
fn vertical_alignment_ratio(heading: (f32, f32)) -> f32 {
    heading.1.abs() / heading.0.abs().max(0.001)
}

fn lobe_hash01(coord: AtlasCoord, cell: MesoGuideCell, salt: u64) -> f32 {
    let hill_bits = ((cell.hilliness.to_bits() as u64) << 32) ^ cell.hill_height.to_bits() as u64;
    let bits = splitmix64(
        salt
            ^ hill_bits
            ^ (coord.x as i64 as u64).wrapping_mul(LOBE_HASH_K1)
            ^ (coord.z as i64 as u64).wrapping_mul(LOBE_HASH_K2)
            ^ LOBE_HASH_K3,
    ) >> 11;
    let max = ((1_u64 << 53) - 1) as f64;
    (bits as f64 / max) as f32
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(LOBE_HASH_K1);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::MesoGuideMap;

    #[test]
    fn hill_cluster_rasterization_forms_an_extended_cluster() {
        let area = crate::world::AtlasArea::new(AtlasCoord::new(0, 0), 16, 16).unwrap();
        let mut cells = crate::world::AtlasGrid::defaulted(area);
        let instance = FeatureInstance {
            kind: MesoFeatureKind::HillCluster,
            center_x: 7.5,
            center_z: 7.5,
            heading_x: 1.0,
            heading_z: 0.0,
            radius_x_cells: 2.2,
            radius_z_cells: 1.5,
            strength_blocks: 11.0,
            spacing_cells: 1.0,
        };
        let normal = (-instance.heading_z, instance.heading_x);

        for coord in area.coords() {
            let cell = cells.get_mut(coord).expect("test raster cell must exist");
            let delta_x = coord.x as f32 + 0.5 - instance.center_x;
            let delta_z = coord.z as f32 + 0.5 - instance.center_z;
            let along = delta_x * instance.heading_x + delta_z * instance.heading_z;
            let across = delta_x * normal.0 + delta_z * normal.1;
            rasterize(instance, cell, along, across);
        }

        let strong_coords = area
            .coords()
            .filter(|coord| {
                let cell = cells.get(*coord).unwrap();
                cell.hilliness >= 0.14 && cell.hill_height >= 1.8
            })
            .collect::<Vec<_>>();
        let min_x = strong_coords.iter().map(|coord| coord.x).min().unwrap();
        let max_x = strong_coords.iter().map(|coord| coord.x).max().unwrap();
        let min_z = strong_coords.iter().map(|coord| coord.z).min().unwrap();
        let max_z = strong_coords.iter().map(|coord| coord.z).max().unwrap();

        assert!(
            strong_coords.len() >= 6,
            "expected hill cluster to cover several strong cells, found {}",
            strong_coords.len()
        );
        assert!(
            max_x - min_x >= 3,
            "expected hill cluster to span multiple cells along heading, got x span {}",
            max_x - min_x
        );
        assert!(
            max_z - min_z >= 1,
            "expected hill cluster to keep non-zero cross-cluster width, got z span {}",
            max_z - min_z
        );
    }

    #[test]
    fn hill_cluster_rasterization_keeps_multiple_strong_masses() {
        let area = crate::world::AtlasArea::new(AtlasCoord::new(0, 0), 16, 16).unwrap();
        let mut cells = crate::world::AtlasGrid::defaulted(area);
        let instance = FeatureInstance {
            kind: MesoFeatureKind::HillCluster,
            center_x: 7.5,
            center_z: 7.5,
            heading_x: 1.0,
            heading_z: 0.0,
            radius_x_cells: 1.8,
            radius_z_cells: 1.2,
            strength_blocks: 11.0,
            spacing_cells: 1.0,
        };
        let normal = (-instance.heading_z, instance.heading_x);

        for coord in area.coords() {
            let cell = cells.get_mut(coord).expect("test raster cell must exist");
            let delta_x = coord.x as f32 + 0.5 - instance.center_x;
            let delta_z = coord.z as f32 + 0.5 - instance.center_z;
            let along = delta_x * instance.heading_x + delta_z * instance.heading_z;
            let across = delta_x * normal.0 + delta_z * normal.1;
            rasterize(instance, cell, along, across);
        }

        let mass_cells = area
            .coords()
            .filter(|coord| {
                let cell = cells.get(*coord).unwrap();
                cell.hill_height >= 2.8
            })
            .count();

        assert!(
            mass_cells >= 2,
            "expected hill cluster to keep multiple strong mass cells, found only {mass_cells}"
        );
    }

    #[test]
    fn dominant_apply_axis_prefers_neighbor_alignment() {
        let meso_span_blocks = (CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32) as f32;
        let mut sources = [GuideSource::default(); MAX_APPLY_SOURCES];
        sources[0] = guide_source(
            AtlasCoord::new(1, 1),
            MesoGuideCell {
                hilliness: 0.90,
                hill_height: 10.0,
                ..MesoGuideCell::default()
            },
            meso_span_blocks,
        )
        .unwrap();
        sources[1] = guide_source(
            AtlasCoord::new(2, 1),
            MesoGuideCell {
                hilliness: 0.84,
                hill_height: 8.9,
                ..MesoGuideCell::default()
            },
            meso_span_blocks,
        )
        .unwrap();
        sources[2] = guide_source(
            AtlasCoord::new(3, 1),
            MesoGuideCell {
                hilliness: 0.78,
                hill_height: 8.0,
                ..MesoGuideCell::default()
            },
            meso_span_blocks,
        )
        .unwrap();

        let heading = dominant_apply_axis(&sources, 3, 2, 1);
        assert!(
            horizontal_alignment_ratio(heading) >= 1.8,
            "expected horizontally aligned sources to prefer a horizontal cluster axis, got {heading:?}"
        );
        assert!(
            anisotropy_ratio(&sources, 3) >= 1.6,
            "expected the synthetic source layout to stay anisotropic"
        );
    }

    #[test]
    fn dominant_apply_axis_handles_vertical_neighbor_alignment() {
        let meso_span_blocks = (CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32) as f32;
        let mut sources = [GuideSource::default(); MAX_APPLY_SOURCES];
        sources[0] = guide_source(
            AtlasCoord::new(2, 1),
            MesoGuideCell {
                hilliness: 0.88,
                hill_height: 9.2,
                ..MesoGuideCell::default()
            },
            meso_span_blocks,
        )
        .unwrap();
        sources[1] = guide_source(
            AtlasCoord::new(2, 2),
            MesoGuideCell {
                hilliness: 0.86,
                hill_height: 8.8,
                ..MesoGuideCell::default()
            },
            meso_span_blocks,
        )
        .unwrap();
        sources[2] = guide_source(
            AtlasCoord::new(2, 3),
            MesoGuideCell {
                hilliness: 0.82,
                hill_height: 8.1,
                ..MesoGuideCell::default()
            },
            meso_span_blocks,
        )
        .unwrap();

        let heading = dominant_apply_axis(&sources, 3, 2, 2);
        assert!(
            vertical_alignment_ratio(heading) >= 1.8,
            "expected vertically aligned sources to prefer a vertical cluster axis, got {heading:?}"
        );
    }

    #[test]
    fn apply_signal_resolves_multiple_macro_lobes_from_neighboring_cells() {
        let area = crate::world::AtlasArea::new(AtlasCoord::new(0, 0), 4, 4).unwrap();
        let mut cells = crate::world::AtlasGrid::defaulted(area);
        *cells.get_mut(AtlasCoord::new(1, 1)).unwrap() = MesoGuideCell {
            hilliness: 0.92,
            hill_height: 9.4,
            ..MesoGuideCell::default()
        };
        *cells.get_mut(AtlasCoord::new(2, 1)).unwrap() = MesoGuideCell {
            hilliness: 0.88,
            hill_height: 8.7,
            ..MesoGuideCell::default()
        };
        let guides = MesoGuideMap { area, cells };
        let origin_world_x = CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32;
        let origin_world_z = CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32;
        let mut local_peak_count = 0;

        for sample_z in (8..120).step_by(8) {
            for sample_x in (8..120).step_by(8) {
                let world_x = origin_world_x + sample_x;
                let world_z = origin_world_z + sample_z;
                let center = sample_apply_signal(&guides, world_x, world_z).lobe_height_blocks;
                if center < 4.5 {
                    continue;
                }

                let north = sample_apply_signal(&guides, world_x, world_z - 8).lobe_height_blocks;
                let south = sample_apply_signal(&guides, world_x, world_z + 8).lobe_height_blocks;
                let west = sample_apply_signal(&guides, world_x - 8, world_z).lobe_height_blocks;
                let east = sample_apply_signal(&guides, world_x + 8, world_z).lobe_height_blocks;
                if center >= north && center >= south && center >= west && center >= east {
                    local_peak_count += 1;
                }
            }
        }

        assert!(
            local_peak_count >= 3,
            "expected neighboring guide cells to resolve as multiple macro lobes, found {local_peak_count}"
        );
    }

    #[test]
    fn apply_signal_keeps_a_broader_shoulder_than_core() {
        let area = crate::world::AtlasArea::new(AtlasCoord::new(0, 0), 4, 4).unwrap();
        let mut cells = crate::world::AtlasGrid::defaulted(area);
        *cells.get_mut(AtlasCoord::new(1, 1)).unwrap() = MesoGuideCell {
            hilliness: 0.91,
            hill_height: 9.8,
            ..MesoGuideCell::default()
        };
        *cells.get_mut(AtlasCoord::new(2, 1)).unwrap() = MesoGuideCell {
            hilliness: 0.87,
            hill_height: 8.9,
            ..MesoGuideCell::default()
        };
        let guides = MesoGuideMap { area, cells };
        let sample = sample_apply_signal(
            &guides,
            CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32 + 20,
            CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32 + 34,
        );

        assert!(sample.lobe_height_blocks > 0.0);
        assert!(
            sample.shoulder_coverage >= sample.coverage,
            "expected hill clusters to keep a broader shoulder than core, got {sample:?}"
        );
    }

    #[test]
    fn macro_lobe_aspect_ratio_stays_below_extreme_chain_values() {
        let meso_span_blocks = (CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32) as f32;
        let source = guide_source(
            AtlasCoord::new(3, 2),
            MesoGuideCell {
                hilliness: 0.94,
                hill_height: 12.8,
                ..MesoGuideCell::default()
            },
            meso_span_blocks,
        )
        .expect("strong test source should build");
        let heading_x = 1.0;
        let heading_z = 0.0;
        let normal_x = 0.0;
        let normal_z = 1.0;
        let major_scale =
            (0.88 + source.cell.hilliness * 0.18 + (source.cell.hill_height / 18.0).clamp(0.0, 0.28))
                .clamp(0.88, 1.34);
        let minor_scale =
            (0.78 + source.cell.hilliness * 0.16 + (source.cell.hill_height / 22.0).clamp(0.0, 0.16))
                .clamp(0.78, 1.12);
        let base_major = lerp_f32(
            39.0,
            62.0,
            lobe_hash01(source.coord, source.cell, SOURCE_MAJOR_RADIUS_SALT),
        ) * major_scale;
        let base_minor = lerp_f32(
            28.0,
            44.0,
            lobe_hash01(source.coord, source.cell, SOURCE_MINOR_RADIUS_SALT),
        ) * minor_scale;
        let chain_spacing = lerp_f32(
            15.0,
            24.0,
            lobe_hash01(source.coord, source.cell, SOURCE_CHAIN_SPACING_SALT),
        ) * (0.92 + source.cell.hilliness * 0.24);
        let lobe_count = 4;
        let chain_span = chain_spacing * (lobe_count - 1) as f32;

        for lobe_index in 0..lobe_count {
            let progress = lobe_index as f32 / (lobe_count - 1) as f32;
            let center_bias = 1.0 - (progress * 2.0 - 1.0).abs();
            let lobe = macro_lobe_descriptor(
                source,
                heading_x,
                heading_z,
                normal_x,
                normal_z,
                base_major,
                base_minor,
                chain_span,
                chain_spacing,
                lobe_index,
                progress,
                center_bias,
            );
            let aspect_ratio = lobe.radius_x_blocks / lobe.radius_z_blocks.max(f32::EPSILON);
            assert!(
                aspect_ratio <= 2.15,
                "expected hill lobes to avoid extreme one-axis stretch, got aspect ratio {aspect_ratio:.3} for lobe {lobe_index}"
            );
        }
    }

}
