use std::f32::consts::TAU;

use crate::world::atlas::{
    AtlasCell, AtlasCoord, MesoGuideCell, MesoGuideMap, sample_meso_guides,
};
use crate::world::{CHUNK_EDGE_I32, MESO_GUIDE_CELL_SIZE_IN_CHUNKS};

use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};
use super::super::{
    FeatureInstance, MesoFeatureKind, cell_center_jitter, ellipse_footprint, hash01, lerp_f32,
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
const SOURCE_KEEPOUT_RADIUS_SALT: u64 = 0xD811_B6D2_2200_000C;
const SOURCE_WARP_ALONG_SALT: u64 = 0xD811_B6D2_2200_000D;
const SOURCE_WARP_ACROSS_SALT: u64 = 0xD811_B6D2_2200_000E;
const SOURCE_SHOULDER_OFFSET_SALT: u64 = 0xD811_B6D2_2200_000F;
const SOURCE_SHOULDER_SIDE_SALT: u64 = 0xD811_B6D2_2200_0010;
const SOURCE_NOTCH_OFFSET_SALT: u64 = 0xD811_B6D2_2200_0011;
const SOURCE_NOTCH_SIDE_SALT: u64 = 0xD811_B6D2_2200_0012;
const SOURCE_NOTCH_STRENGTH_SALT: u64 = 0xD811_B6D2_2200_0013;
const APPLY_SCAN_RADIUS_CELLS: i32 = 5;
const MAX_APPLY_SOURCES: usize = 121;
const MAX_RESOLVED_CLUSTER_SOURCES: usize = 4;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HillClusterApplySample {
    pub coverage: f32,
    pub shoulder_coverage: f32,
    pub lobe_height_blocks: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HillClusterSurfaceSample {
    pub target_surface_y: f32,
    pub blend_weight: f32,
    pub relief_spend: f32,
    pub core_coverage: f32,
    pub shoulder_coverage: f32,
}

impl HillClusterSurfaceSample {
    pub fn flat(base_surface_y: f32) -> Self {
        Self {
            target_surface_y: base_surface_y,
            blend_weight: 0.0,
            relief_spend: 0.0,
            core_coverage: 0.0,
            shoulder_coverage: 0.0,
        }
    }
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
    keepout_radius_blocks: f32,
}

impl Default for GuideSource {
    fn default() -> Self {
        Self {
            coord: AtlasCoord::new(0, 0),
            cell: MesoGuideCell::default(),
            center_x: 0.0,
            center_z: 0.0,
            weight: 0.0,
            keepout_radius_blocks: 0.0,
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
    let mut candidates = Vec::with_capacity(MAX_APPLY_SOURCES);
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
            if !is_local_source_peak(guides, coord, cell) {
                continue;
            }
            let Some(source) = guide_source(coord, cell, meso_span_blocks as f32) else {
                continue;
            };
            candidates.push(source);
        }
    }

    if candidates.is_empty() {
        return HillClusterApplySample::default();
    }

    let sources = prune_cluster_sources(candidates, sample_x, sample_z);
    if sources.is_empty() {
        return HillClusterApplySample::default();
    }

    let cluster_heading = dominant_apply_axis(&sources, base_cell_x, base_cell_z);

    for source in &sources {
        let (heading_x, heading_z) = source_chain_heading(*source, cluster_heading);
        let normal_x = -heading_z;
        let normal_z = heading_x;
        let major_scale =
            (0.94 + source.cell.hilliness * 0.22 + (source.cell.hill_height / 16.0).clamp(0.0, 0.36))
                .clamp(0.94, 1.52);
        let minor_scale =
            (0.92 + source.cell.hilliness * 0.18 + (source.cell.hill_height / 20.0).clamp(0.0, 0.20))
                .clamp(0.92, 1.26);
        let base_major = lerp_f32(
            52.0,
            86.0,
            lobe_hash01(source.coord, source.cell, SOURCE_MAJOR_RADIUS_SALT),
        ) * major_scale;
        let base_minor = lerp_f32(
            38.0,
            62.0,
            lobe_hash01(source.coord, source.cell, SOURCE_MINOR_RADIUS_SALT),
        ) * minor_scale;
        let chain_spacing = lerp_f32(
            24.0,
            38.0,
            lobe_hash01(source.coord, source.cell, SOURCE_CHAIN_SPACING_SALT),
        ) * (1.02 + source.cell.hilliness * 0.18);
        let lobe_count = if lobe_hash01(source.coord, source.cell, SOURCE_COUNT_SALT) >= 0.70
            || source.cell.hilliness >= 0.86
            || source.cell.hill_height >= 12.4
        {
            3
        } else {
            2
        };
        let chain_span = chain_spacing * (lobe_count - 1) as f32;
        let delta_x = sample_x - source.center_x;
        let delta_z = sample_z - source.center_z;
        let source_along = delta_x * heading_x + delta_z * heading_z;
        let source_across = delta_x * normal_x + delta_z * normal_z;
        let shoulder = ellipse_footprint(
            source_along,
            source_across,
            base_major + chain_span * 1.16 + 12.0,
            base_minor * 2.18 + 16.0,
        );
        shoulder_coverage = shoulder_coverage.max(
            (shoulder * (0.20 + source.cell.hilliness * 0.32)).clamp(0.0, 0.90),
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
            let footprint = irregular_lobe_footprint(lobe, *source, lobe_index, sample_x, sample_z);
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

pub(crate) fn sample_surface(
    guides: &MesoGuideMap,
    world_x: i32,
    world_z: i32,
    base_surface_y: f32,
    relief_budget: f32,
) -> HillClusterSurfaceSample {
    let apply = sample_apply_signal(guides, world_x, world_z);
    let coverage = apply.coverage.clamp(0.0, 1.0);
    let shoulder = apply.shoulder_coverage.max(coverage).clamp(0.0, 1.0);
    if shoulder <= f32::EPSILON || apply.lobe_height_blocks <= f32::EPSILON {
        return HillClusterSurfaceSample::flat(base_surface_y);
    }

    let meso = sample_meso_guides(guides, world_x, world_z);
    let shoulder_raise = meso.hill_height
        * (0.42 + meso.hilliness * 0.24 + shoulder * 0.26)
        * smoothstep_range(0.06, 0.98, shoulder);
    let core_raise = apply.lobe_height_blocks
        * (0.96 + meso.hilliness * 0.18 + coverage * 0.16)
        * smoothstep_range(0.04, 0.90, coverage);
    let raw_target_raise = shoulder_raise * (0.76 + shoulder * 0.20) + core_raise;
    let target_raise = soft_cap_positive(raw_target_raise, (relief_budget * 1.22).max(11.5));
    if target_raise <= f32::EPSILON {
        return HillClusterSurfaceSample::flat(base_surface_y);
    }

    let blend_weight = smoothstep01(
        (shoulder * 0.78 + coverage * 0.26 + meso.hilliness * 0.10).clamp(0.0, 1.0),
    );

    HillClusterSurfaceSample {
        target_surface_y: base_surface_y + target_raise,
        blend_weight,
        relief_spend: target_raise * blend_weight,
        core_coverage: coverage,
        shoulder_coverage: shoulder,
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
    if cell.hilliness < 0.30 || cell.hill_height < 3.8 {
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
    let weight = (0.18 + cell.hilliness * 0.68 + (cell.hill_height / 16.0).clamp(0.0, 0.56))
        .clamp(0.18, 1.52);
    let keepout_radius_blocks = meso_span_blocks
        * lerp_f32(
            0.72,
            1.18,
            lobe_hash01(coord, cell, SOURCE_KEEPOUT_RADIUS_SALT),
        )
        * (0.84 + cell.hilliness * 0.34 + (cell.hill_height / 20.0).clamp(0.0, 0.24));

    Some(GuideSource {
        coord,
        cell,
        center_x,
        center_z,
        weight,
        keepout_radius_blocks,
    })
}

fn dominant_apply_axis(
    sources: &[GuideSource],
    base_cell_x: i32,
    base_cell_z: i32,
) -> (f32, f32) {
    let mut total_weight = 0.0_f32;
    let mut mean_x = 0.0_f32;
    let mut mean_z = 0.0_f32;

    for source in sources {
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

    for source in sources {
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
        -0.34,
        0.34,
        lobe_hash01(source.coord, source.cell, SOURCE_AXIS_JITTER_SALT),
    ) * (0.60 + source.cell.hilliness * 0.16);
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
        -chain_spacing * 0.16,
        chain_spacing * 0.16,
        indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_ALONG_JITTER_SALT),
    );
    let patterned_side = match lobe_index {
        0 => -0.22,
        1 => 0.18,
        _ => -0.08,
    };
    let side_jitter = lerp_f32(
        -base_minor * 0.30,
        base_minor * 0.30,
        indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_SIDE_JITTER_SALT),
    );
    let offset_along = (progress - 0.5) * chain_span + along_jitter;
    let offset_across = base_minor * patterned_side + side_jitter;
    let radius_x = base_major
        * (0.98 + center_bias * 0.20 + source.cell.hilliness * 0.10)
        * lerp_f32(
            1.00,
            1.22,
            indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_MAJOR_RADIUS_SALT),
        );
    let radius_z = base_minor
        * (1.04 + center_bias * 0.14 + source.cell.hilliness * 0.08)
        * lerp_f32(
            1.12,
            1.36,
            indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_MINOR_RADIUS_SALT),
        );
    let height_blocks = source.cell.hill_height
        * (1.26 + source.cell.hilliness * 0.38 + center_bias * 0.24)
        * lerp_f32(
            1.14,
            1.72,
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

fn is_local_source_peak(guides: &MesoGuideMap, coord: AtlasCoord, cell: MesoGuideCell) -> bool {
    if cell.hilliness >= 0.92 || cell.hill_height >= 12.5 {
        return true;
    }

    for neighbor_z in (coord.z - 1)..=(coord.z + 1) {
        for neighbor_x in (coord.x - 1)..=(coord.x + 1) {
            if neighbor_x == coord.x && neighbor_z == coord.z {
                continue;
            }

            let Some(neighbor) = guides.cells().get(AtlasCoord::new(neighbor_x, neighbor_z)).copied() else {
                continue;
            };
            if neighbor.hilliness < 0.24 || neighbor.hill_height < 3.2 {
                continue;
            }

            let stronger_height = neighbor.hill_height >= cell.hill_height + 0.8;
            let comparable_hilliness = neighbor.hilliness >= cell.hilliness - 0.04;
            if stronger_height && comparable_hilliness {
                return false;
            }
        }
    }

    true
}

fn prune_cluster_sources(
    mut candidates: Vec<GuideSource>,
    sample_x: f32,
    sample_z: f32,
) -> Vec<GuideSource> {
    candidates.sort_by(|a, b| {
        let a_score = source_relevance_score(*a, sample_x, sample_z);
        let b_score = source_relevance_score(*b, sample_x, sample_z);
        b_score.total_cmp(&a_score)
    });
    let mut kept: Vec<GuideSource> = Vec::with_capacity(MAX_RESOLVED_CLUSTER_SOURCES);

    for candidate in candidates {
        let relevance = source_relevance_score(candidate, sample_x, sample_z);
        if relevance <= 0.04 {
            continue;
        }

        let mut overlaps = false;
        for existing in &kept {
            let separation = distance_between_points(
                (candidate.center_x, candidate.center_z),
                (existing.center_x, existing.center_z),
            );
            let minimum = candidate
                .keepout_radius_blocks
                .min(existing.keepout_radius_blocks)
                * lerp_f32(0.60, 0.92, relevance);
            if separation < minimum {
                overlaps = true;
                break;
            }
        }

        if overlaps {
            continue;
        }

        kept.push(candidate);
        if kept.len() >= MAX_RESOLVED_CLUSTER_SOURCES {
            break;
        }
    }

    kept
}

fn source_relevance_score(source: GuideSource, sample_x: f32, sample_z: f32) -> f32 {
    let distance = distance_between_points((source.center_x, source.center_z), (sample_x, sample_z));
    let distance_weight = smoothstep_range(source.keepout_radius_blocks * 4.6, 0.0, distance);
    source.weight * (0.16 + distance_weight * 0.84)
}

fn irregular_lobe_footprint(
    lobe: MacroLobeDescriptor,
    source: GuideSource,
    lobe_index: usize,
    sample_x: f32,
    sample_z: f32,
) -> f32 {
    let delta_x = sample_x - lobe.center_x;
    let delta_z = sample_z - lobe.center_z;
    let along = delta_x * lobe.heading_x + delta_z * lobe.heading_z;
    let across = delta_x * -lobe.heading_z + delta_z * lobe.heading_x;
    let along_phase =
        indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_WARP_ALONG_SALT) * TAU;
    let across_phase =
        indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_WARP_ACROSS_SALT) * TAU;
    let warped_along = along
        + (((across / lobe.radius_z_blocks.max(1.0)) * 1.55) + along_phase).sin()
            * lobe.radius_x_blocks
            * lerp_f32(
                0.06,
                0.16,
                indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_WARP_ALONG_SALT),
            );
    let warped_across = across
        + (((along / lobe.radius_x_blocks.max(1.0)) * 1.85) + across_phase).sin()
            * lobe.radius_z_blocks
            * lerp_f32(
                0.10,
                0.22,
                indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_WARP_ACROSS_SALT),
            );
    let core = ellipse_footprint(
        warped_along,
        warped_across,
        lobe.radius_x_blocks.max(f32::EPSILON),
        lobe.radius_z_blocks.max(f32::EPSILON),
    );
    let shoulder_offset = lerp_f32(
        -0.18,
        0.26,
        indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_SHOULDER_OFFSET_SALT),
    );
    let shoulder_side = lerp_f32(
        -0.24,
        0.30,
        indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_SHOULDER_SIDE_SALT),
    );
    let shoulder = ellipse_footprint(
        warped_along - lobe.radius_x_blocks * shoulder_offset,
        warped_across + lobe.radius_z_blocks * shoulder_side,
        lobe.radius_x_blocks * 0.82,
        lobe.radius_z_blocks * 0.94,
    );
    let backfill = ellipse_footprint(
        warped_along + lobe.radius_x_blocks * 0.14,
        warped_across - lobe.radius_z_blocks * 0.10,
        lobe.radius_x_blocks * 0.66,
        lobe.radius_z_blocks * 0.74,
    );
    let notch = ellipse_footprint(
        warped_along
            + lobe.radius_x_blocks
                * lerp_f32(
                    -0.10,
                    0.22,
                    indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_NOTCH_OFFSET_SALT),
                ),
        warped_across
            + lobe.radius_z_blocks
                * lerp_f32(
                    -0.34,
                    0.34,
                    indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_NOTCH_SIDE_SALT),
                ),
        lobe.radius_x_blocks * 0.28,
        lobe.radius_z_blocks * 0.24,
    ) * lerp_f32(
        0.14,
        0.30,
        indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_NOTCH_STRENGTH_SALT),
    );

    (core * 0.72 + shoulder * 0.30 + backfill * 0.20 - notch).clamp(0.0, 1.0)
}

fn distance_between_points(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dz = a.1 - b.1;
    (dx * dx + dz * dz).sqrt()
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
fn source_weight_sum(sources: &[GuideSource]) -> f32 {
    sources.iter().map(|source| source.weight).sum::<f32>()
}

#[cfg(test)]
fn anisotropy_ratio(sources: &[GuideSource]) -> f32 {
    let total_weight = source_weight_sum(sources);
    if total_weight <= f32::EPSILON {
        return 1.0;
    }

    let mean_x = sources
        .iter()
        .map(|source| source.center_x * source.weight)
        .sum::<f32>()
        / total_weight;
    let mean_z = sources
        .iter()
        .map(|source| source.center_z * source.weight)
        .sum::<f32>()
        / total_weight;
    let mut xx = 0.0_f32;
    let mut zz = 0.0_f32;

    for source in sources {
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

fn smoothstep01(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn smoothstep_range(edge0: f32, edge1: f32, value: f32) -> f32 {
    if (edge1 - edge0).abs() <= f32::EPSILON {
        return if value >= edge1 { 1.0 } else { 0.0 };
    }

    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn soft_cap_positive(value: f32, cap: f32) -> f32 {
    if value <= 0.0 {
        return 0.0;
    }

    let safe_cap = cap.max(f32::EPSILON);
    safe_cap * (1.0 - (-value / safe_cap).exp())
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
        let sources = vec![
            guide_source(
            AtlasCoord::new(1, 1),
            MesoGuideCell {
                hilliness: 0.90,
                hill_height: 10.0,
                ..MesoGuideCell::default()
            },
            meso_span_blocks,
        )
        .unwrap(),
            guide_source(
            AtlasCoord::new(2, 1),
            MesoGuideCell {
                hilliness: 0.84,
                hill_height: 8.9,
                ..MesoGuideCell::default()
            },
            meso_span_blocks,
        )
        .unwrap(),
            guide_source(
            AtlasCoord::new(3, 1),
            MesoGuideCell {
                hilliness: 0.78,
                hill_height: 8.0,
                ..MesoGuideCell::default()
            },
            meso_span_blocks,
        )
        .unwrap(),
        ];

        let heading = dominant_apply_axis(&sources, 2, 1);
        assert!(
            horizontal_alignment_ratio(heading) >= 1.8,
            "expected horizontally aligned sources to prefer a horizontal cluster axis, got {heading:?}"
        );
        assert!(
            anisotropy_ratio(&sources) >= 1.6,
            "expected the synthetic source layout to stay anisotropic"
        );
    }

    #[test]
    fn dominant_apply_axis_handles_vertical_neighbor_alignment() {
        let meso_span_blocks = (CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32) as f32;
        let sources = vec![
            guide_source(
            AtlasCoord::new(2, 1),
            MesoGuideCell {
                hilliness: 0.88,
                hill_height: 9.2,
                ..MesoGuideCell::default()
            },
            meso_span_blocks,
        )
        .unwrap(),
            guide_source(
            AtlasCoord::new(2, 2),
            MesoGuideCell {
                hilliness: 0.86,
                hill_height: 8.8,
                ..MesoGuideCell::default()
            },
            meso_span_blocks,
        )
        .unwrap(),
            guide_source(
            AtlasCoord::new(2, 3),
            MesoGuideCell {
                hilliness: 0.82,
                hill_height: 8.1,
                ..MesoGuideCell::default()
            },
            meso_span_blocks,
        )
        .unwrap(),
        ];

        let heading = dominant_apply_axis(&sources, 2, 2);
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
        assert!(
            local_peak_count <= 8,
            "expected source pruning to avoid cluttered tiny-hill overpopulation, found {local_peak_count}"
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
            52.0,
            86.0,
            lobe_hash01(source.coord, source.cell, SOURCE_MAJOR_RADIUS_SALT),
        ) * major_scale;
        let base_minor = lerp_f32(
            38.0,
            62.0,
            lobe_hash01(source.coord, source.cell, SOURCE_MINOR_RADIUS_SALT),
        ) * minor_scale;
        let chain_spacing = lerp_f32(
            24.0,
            38.0,
            lobe_hash01(source.coord, source.cell, SOURCE_CHAIN_SPACING_SALT),
        ) * (1.02 + source.cell.hilliness * 0.18);
        let lobe_count = 3;
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
                aspect_ratio <= 2.10,
                "expected hill lobes to avoid extreme one-axis stretch, got aspect ratio {aspect_ratio:.3} for lobe {lobe_index}"
            );
        }
    }

    #[test]
    fn prune_cluster_sources_reduces_dense_neighbor_overlap() {
        let meso_span_blocks = (CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32) as f32;
        let candidates = vec![
            guide_source(
                AtlasCoord::new(1, 1),
                MesoGuideCell {
                    hilliness: 0.94,
                    hill_height: 12.0,
                    ..MesoGuideCell::default()
                },
                meso_span_blocks,
            )
            .unwrap(),
            guide_source(
                AtlasCoord::new(2, 1),
                MesoGuideCell {
                    hilliness: 0.90,
                    hill_height: 11.2,
                    ..MesoGuideCell::default()
                },
                meso_span_blocks,
            )
            .unwrap(),
            guide_source(
                AtlasCoord::new(1, 2),
                MesoGuideCell {
                    hilliness: 0.88,
                    hill_height: 10.6,
                    ..MesoGuideCell::default()
                },
                meso_span_blocks,
            )
            .unwrap(),
        ];

        let kept = prune_cluster_sources(candidates, meso_span_blocks * 1.5, meso_span_blocks * 1.5);
        assert!(
            kept.len() <= 2,
            "expected dense neighboring hill sources to prune down, kept {} sources",
            kept.len()
        );
    }

    #[test]
    fn apply_signal_is_stable_across_meso_cell_boundaries() {
        let area = crate::world::AtlasArea::new(AtlasCoord::new(0, 0), 6, 6).unwrap();
        let mut cells = crate::world::AtlasGrid::defaulted(area);
        for coord in area.coords() {
            if (coord.x + coord.z) % 2 == 0 {
                *cells.get_mut(coord).unwrap() = MesoGuideCell {
                    hilliness: 0.74 + (coord.x as f32 * 0.03).clamp(0.0, 0.16),
                    hill_height: 7.0 + (coord.z as f32 * 0.7).clamp(0.0, 3.0),
                    ..MesoGuideCell::default()
                };
            }
        }
        let guides = MesoGuideMap { area, cells };
        let meso_span = CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32;
        let world_z = meso_span * 2 + 24;
        let left = sample_apply_signal(&guides, meso_span * 2 - 1, world_z);
        let right = sample_apply_signal(&guides, meso_span * 2, world_z);
        let seam_delta = (left.lobe_height_blocks - right.lobe_height_blocks).abs();

        assert!(
            seam_delta <= 3.6,
            "expected hill-cluster apply support to stay stable across meso-cell boundaries, seam delta={seam_delta:.3}\nleft={left:?}\nright={right:?}"
        );
    }

    #[test]
    fn irregular_lobe_footprint_breaks_mirror_symmetry() {
        let meso_span_blocks = (CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32) as f32;
        let source = guide_source(
            AtlasCoord::new(3, 2),
            MesoGuideCell {
                hilliness: 0.91,
                hill_height: 11.4,
                ..MesoGuideCell::default()
            },
            meso_span_blocks,
        )
        .expect("strong source should build");
        let lobe = macro_lobe_descriptor(
            source,
            1.0,
            0.0,
            0.0,
            1.0,
            72.0,
            48.0,
            28.0,
            28.0,
            0,
            0.0,
            0.0,
        );
        let a =
            irregular_lobe_footprint(lobe, source, 0, lobe.center_x + 34.0, lobe.center_z + 18.0);
        let b =
            irregular_lobe_footprint(lobe, source, 0, lobe.center_x - 34.0, lobe.center_z - 18.0);

        assert!(
            (a - b).abs() >= 0.04,
            "expected irregular hill lobes to avoid mirror-symmetric blobs, got a={a:.3}, b={b:.3}"
        );
    }

    #[test]
    fn sample_surface_returns_flat_when_guides_are_absent() {
        let area = crate::world::AtlasArea::new(AtlasCoord::new(0, 0), 4, 4).unwrap();
        let cells = crate::world::AtlasGrid::defaulted(area);
        let guides = MesoGuideMap { area, cells };
        let sample = sample_surface(&guides, 96, 96, 100.0, 12.0);

        assert_eq!(sample, HillClusterSurfaceSample::flat(100.0));
    }

    #[test]
    fn sample_surface_can_raise_strong_peak_more_than_eight_blocks() {
        let area = crate::world::AtlasArea::new(AtlasCoord::new(0, 0), 4, 4).unwrap();
        let mut cells = crate::world::AtlasGrid::defaulted(area);
        *cells.get_mut(AtlasCoord::new(1, 1)).unwrap() = MesoGuideCell {
            hilliness: 0.95,
            hill_height: 11.8,
            ..MesoGuideCell::default()
        };
        *cells.get_mut(AtlasCoord::new(2, 1)).unwrap() = MesoGuideCell {
            hilliness: 0.90,
            hill_height: 10.6,
            ..MesoGuideCell::default()
        };
        let guides = MesoGuideMap { area, cells };
        let origin_world = CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32;
        let mut strongest = HillClusterSurfaceSample::flat(100.0);

        for sample_z in (8..120).step_by(4) {
            for sample_x in (8..120).step_by(4) {
                let sample = sample_surface(
                    &guides,
                    origin_world + sample_x,
                    origin_world + sample_z,
                    100.0,
                    12.0,
                );
                if sample.target_surface_y > strongest.target_surface_y {
                    strongest = sample;
                }
            }
        }

        assert!(
            strongest.target_surface_y - 100.0 >= 8.0,
            "expected strong hill-cluster surface resolve to exceed +8 blocks above base, got {:?}",
            strongest
        );
        assert!(
            strongest.blend_weight >= 0.55,
            "expected strong hill-cluster peak to keep meaningful blend support, got {:?}",
            strongest
        );
    }

}
