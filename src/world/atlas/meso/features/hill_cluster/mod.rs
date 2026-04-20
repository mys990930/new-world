use std::f32::consts::TAU;

use crate::world::atlas::{AtlasCell, AtlasCoord, MesoGuideCell, MesoGuideMap};
use crate::world::{CHUNK_EDGE_I32, MESO_GUIDE_CELL_SIZE_IN_CHUNKS};

use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};
use super::super::{
    FeatureInstance, MesoFeatureKind, cell_center_jitter, ellipse_footprint, hash01, lerp_f32,
    smoothstep_range,
};

const STRENGTH_MIN_BLOCKS: f32 = 4.6;
const STRENGTH_MAX_BLOCKS: f32 = 10.8;
const HILLLET_HASH_K1: u64 = 0x9E37_79B9_7F4A_7C15;
const HILLLET_HASH_K2: u64 = 0xC2B2_AE3D_27D4_EB4F;
const HILLLET_HASH_K3: u64 = 0x1656_67B1_9E37_79F9;
const HILLLET_ANGLE_SALT: u64 = 0xD811_B6D2_2100_0001;
const HILLLET_ORBIT_SALT: u64 = 0xD811_B6D2_2100_0002;
const HILLLET_RADIUS_SALT: u64 = 0xD811_B6D2_2100_0003;
const HILLLET_HEIGHT_SALT: u64 = 0xD811_B6D2_2100_0004;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HillClusterApplySample {
    pub peak_mask: f32,
    pub peak_height_blocks: f32,
}

#[derive(Debug, Clone, Copy)]
struct HillletDescriptor {
    center_x: f32,
    center_z: f32,
    radius_blocks: f32,
    height_blocks: f32,
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
        (0.84 + sample.macro_elevation * 0.24 + sample.ruggedness * 0.12).clamp(0.84, 1.22);

    FeatureInstance {
        kind: MesoFeatureKind::HillCluster,
        center_x,
        center_z,
        heading_x: heading.0,
        heading_z: heading.1,
        radius_x_cells: lerp_f32(
            2.0,
            3.9,
            hash01(
                seed,
                cell_coord.x as i64,
                cell_coord.z as i64,
                super::super::MESO_FEATURE_RADIUS_X_SALT,
            ),
        ),
        radius_z_cells: lerp_f32(
            1.6,
            3.0,
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
        instance.radius_x_cells * 1.16,
        instance.radius_z_cells * 1.10,
    );
    if envelope <= 0.0 {
        return;
    }

    let peaks = cluster_peak_footprints(instance, along, across);
    let strongest_peak = peaks
        .into_iter()
        .fold(0.0_f32, f32::max);
    let weighted_peak_sum = peaks[0] * 0.84 + peaks[1] * 1.00 + peaks[2] * 0.80 + peaks[3] * 0.66;
    let shoulder_fill = smoothstep_range(0.92, 0.10, envelope) * 0.28;
    let hilliness = (envelope * 0.24 + strongest_peak * 0.68 + weighted_peak_sum * 0.08)
        .clamp(0.0, 1.0);
    let height_scale = (envelope * 0.18 + strongest_peak * 0.76 + weighted_peak_sum * 0.12 + shoulder_fill)
        .clamp(0.0, 1.18);

    cell.hilliness = (cell.hilliness + hilliness * 0.92).clamp(0.0, 1.0);
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
    let mut strongest_height = 0.0_f32;
    let mut second_height = 0.0_f32;
    let mut strongest_mask = 0.0_f32;

    for cell_z in (base_cell_z - 2)..=(base_cell_z + 2) {
        for cell_x in (base_cell_x - 2)..=(base_cell_x + 2) {
            let coord = AtlasCoord::new(cell_x, cell_z);
            let Some(cell) = guides.cells().get(coord).copied() else {
                continue;
            };
            let hilllet_count = hilllet_count_for_cell(cell);
            if hilllet_count == 0 {
                continue;
            }

            for hilllet_index in 0..hilllet_count {
                let hilllet = hilllet_descriptor(coord, cell, hilllet_index, meso_span_blocks as f32);
                let delta_x = sample_x - hilllet.center_x;
                let delta_z = sample_z - hilllet.center_z;
                let distance = (delta_x * delta_x + delta_z * delta_z).sqrt();
                let footprint =
                    smoothstep_range(1.06, 0.0, distance / hilllet.radius_blocks.max(f32::EPSILON));
                if footprint <= 0.0 {
                    continue;
                }

                let contribution = hilllet.height_blocks * footprint;
                if contribution > strongest_height {
                    second_height = strongest_height;
                    strongest_height = contribution;
                } else if contribution > second_height {
                    second_height = contribution;
                }

                let mask = (footprint * (0.42 + cell.hilliness * 0.58)).clamp(0.0, 1.0);
                strongest_mask = strongest_mask.max(mask);
            }
        }
    }

    if strongest_height <= f32::EPSILON {
        return HillClusterApplySample::default();
    }

    HillClusterApplySample {
        peak_mask: strongest_mask,
        peak_height_blocks: strongest_height + second_height * 0.44,
    }
}

fn cluster_peak_footprints(instance: FeatureInstance, along: f32, across: f32) -> [f32; 4] {
    [
        ellipse_footprint(
            along + instance.radius_x_cells * 0.58,
            across - instance.radius_z_cells * 0.18,
            instance.radius_x_cells * 0.28,
            instance.radius_z_cells * 0.34,
        ),
        ellipse_footprint(
            along + instance.radius_x_cells * 0.02,
            across + instance.radius_z_cells * 0.54,
            instance.radius_x_cells * 0.26,
            instance.radius_z_cells * 0.30,
        ),
        ellipse_footprint(
            along - instance.radius_x_cells * 0.56,
            across - instance.radius_z_cells * 0.20,
            instance.radius_x_cells * 0.29,
            instance.radius_z_cells * 0.34,
        ),
        ellipse_footprint(
            along - instance.radius_x_cells * 0.06,
            across - instance.radius_z_cells * 0.58,
            instance.radius_x_cells * 0.22,
            instance.radius_z_cells * 0.26,
        ),
    ]
}

fn hilllet_count_for_cell(cell: MesoGuideCell) -> u32 {
    let mut count = 0;
    if cell.hilliness >= 0.22 && cell.hill_height >= 2.0 {
        count += 1;
    }
    if cell.hilliness >= 0.52 && cell.hill_height >= 4.4 {
        count += 1;
    }
    if cell.hilliness >= 0.80 && cell.hill_height >= 6.8 {
        count += 1;
    }
    count
}

fn hilllet_descriptor(
    coord: AtlasCoord,
    cell: MesoGuideCell,
    hilllet_index: u32,
    meso_span_blocks: f32,
) -> HillletDescriptor {
    let angle = hilllet_hash01(coord, cell, hilllet_index, HILLLET_ANGLE_SALT) * TAU;
    let orbit_hash = hilllet_hash01(coord, cell, hilllet_index, HILLLET_ORBIT_SALT);
    let orbit = match hilllet_index {
        0 => lerp_f32(0.0, meso_span_blocks * 0.12, orbit_hash),
        1 => lerp_f32(meso_span_blocks * 0.12, meso_span_blocks * 0.24, orbit_hash),
        _ => lerp_f32(meso_span_blocks * 0.18, meso_span_blocks * 0.30, orbit_hash),
    };
    let center_x = coord.x as f32 * meso_span_blocks
        + meso_span_blocks * 0.5
        + angle.cos() * orbit;
    let center_z = coord.z as f32 * meso_span_blocks
        + meso_span_blocks * 0.5
        + angle.sin() * orbit;
    let radius_scale =
        (0.86 + cell.hilliness * 0.18 + (cell.hill_height / 12.0).clamp(0.0, 0.22)).clamp(0.86, 1.26);
    let height_scale = (0.76 + cell.hilliness * 0.22).clamp(0.76, 1.06);

    HillletDescriptor {
        center_x,
        center_z,
        radius_blocks: lerp_f32(
            8.0,
            17.0,
            hilllet_hash01(coord, cell, hilllet_index, HILLLET_RADIUS_SALT),
        ) * radius_scale,
        height_blocks: cell.hill_height
            * lerp_f32(
                0.54,
                1.02,
                hilllet_hash01(coord, cell, hilllet_index, HILLLET_HEIGHT_SALT),
            )
            * height_scale,
    }
}

fn hilllet_hash01(coord: AtlasCoord, cell: MesoGuideCell, hilllet_index: u32, salt: u64) -> f32 {
    let hill_bits = ((cell.hilliness.to_bits() as u64) << 32) ^ cell.hill_height.to_bits() as u64;
    let bits = splitmix64(
        salt
            ^ hill_bits
            ^ (coord.x as i64 as u64).wrapping_mul(HILLLET_HASH_K1)
            ^ (coord.z as i64 as u64).wrapping_mul(HILLLET_HASH_K2)
            ^ (hilllet_index as u64).wrapping_mul(HILLLET_HASH_K3),
    ) >> 11;
    let max = ((1_u64 << 53) - 1) as f64;
    (bits as f64 / max) as f32
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(HILLLET_HASH_K1);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::MesoGuideMap;

    #[test]
    fn hill_cluster_rasterization_creates_multiple_local_peaks() {
        let area = crate::world::AtlasArea::new(AtlasCoord::new(0, 0), 16, 16).unwrap();
        let mut cells = crate::world::AtlasGrid::defaulted(area);
        let instance = FeatureInstance {
            kind: MesoFeatureKind::HillCluster,
            center_x: 7.5,
            center_z: 7.5,
            heading_x: 1.0,
            heading_z: 0.0,
            radius_x_cells: 3.8,
            radius_z_cells: 2.8,
            strength_blocks: 8.0,
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

        let mut local_peak_count = 0;
        for z in 1..15 {
            for x in 1..15 {
                let coord = AtlasCoord::new(x, z);
                let height = cells.get(coord).unwrap().hill_height;
                if height < 5.2 {
                    continue;
                }
                let north = cells.get(AtlasCoord::new(x, z - 1)).unwrap().hill_height;
                let south = cells.get(AtlasCoord::new(x, z + 1)).unwrap().hill_height;
                let west = cells.get(AtlasCoord::new(x - 1, z)).unwrap().hill_height;
                let east = cells.get(AtlasCoord::new(x + 1, z)).unwrap().hill_height;
                if height >= north && height >= south && height >= west && height >= east {
                    local_peak_count += 1;
                }
            }
        }

        assert!(
            local_peak_count >= 3,
            "expected at least three local peaks, found {local_peak_count}"
        );
    }

    #[test]
    fn chunk_space_signal_creates_multiple_local_hilllets() {
        let area = crate::world::AtlasArea::new(AtlasCoord::new(0, 0), 4, 4).unwrap();
        let mut cells = crate::world::AtlasGrid::defaulted(area);
        *cells.get_mut(AtlasCoord::new(1, 1)).unwrap() = MesoGuideCell {
            hilliness: 0.94,
            hill_height: 8.2,
            ..MesoGuideCell::default()
        };
        *cells.get_mut(AtlasCoord::new(2, 1)).unwrap() = MesoGuideCell {
            hilliness: 0.76,
            hill_height: 6.6,
            ..MesoGuideCell::default()
        };
        let guides = MesoGuideMap { area, cells };
        let mut local_peak_count = 0;
        let origin_world_x = CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32;
        let origin_world_z = CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32;

        for sample_z in (8..120).step_by(4) {
            for sample_x in (8..120).step_by(4) {
                let world_x = origin_world_x + sample_x;
                let world_z = origin_world_z + sample_z;
                let center = sample_apply_signal(&guides, world_x, world_z).peak_height_blocks;
                if center < 4.5 {
                    continue;
                }

                let north =
                    sample_apply_signal(&guides, world_x, world_z - 4).peak_height_blocks;
                let south =
                    sample_apply_signal(&guides, world_x, world_z + 4).peak_height_blocks;
                let west =
                    sample_apply_signal(&guides, world_x - 4, world_z).peak_height_blocks;
                let east =
                    sample_apply_signal(&guides, world_x + 4, world_z).peak_height_blocks;
                if center >= north && center >= south && center >= west && center >= east {
                    local_peak_count += 1;
                }
            }
        }

        assert!(
            local_peak_count >= 3,
            "expected multiple chunk-space hilllets, found {local_peak_count}"
        );
    }
}
