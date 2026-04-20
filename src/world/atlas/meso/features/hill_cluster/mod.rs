use std::f32::consts::TAU;

use crate::world::atlas::{AtlasCell, AtlasCoord, MesoGuideCell, MesoGuideMap};
use crate::world::{CHUNK_EDGE_I32, MESO_GUIDE_CELL_SIZE_IN_CHUNKS};

use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};
use super::super::{
    FeatureInstance, MesoFeatureKind, cell_center_jitter, ellipse_footprint, hash01, lerp_f32,
    smoothstep_range,
};

const STRENGTH_MIN_BLOCKS: f32 = 6.4;
const STRENGTH_MAX_BLOCKS: f32 = 14.8;
const LOBE_HASH_K1: u64 = 0x9E37_79B9_7F4A_7C15;
const LOBE_HASH_K2: u64 = 0xC2B2_AE3D_27D4_EB4F;
const LOBE_HASH_K3: u64 = 0x1656_67B1_9E37_79F9;
const LOBE_ANGLE_SALT: u64 = 0xD811_B6D2_2200_0001;
const LOBE_JITTER_X_SALT: u64 = 0xD811_B6D2_2200_0002;
const LOBE_JITTER_Z_SALT: u64 = 0xD811_B6D2_2200_0003;
const LOBE_RADIUS_X_SALT: u64 = 0xD811_B6D2_2200_0004;
const LOBE_RADIUS_Z_SALT: u64 = 0xD811_B6D2_2200_0005;
const LOBE_HEIGHT_SALT: u64 = 0xD811_B6D2_2200_0006;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HillClusterApplySample {
    pub coverage: f32,
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
            1.3,
            2.3,
            hash01(
                seed,
                cell_coord.x as i64,
                cell_coord.z as i64,
                super::super::MESO_FEATURE_RADIUS_X_SALT,
            ),
        ),
        radius_z_cells: lerp_f32(
            1.1,
            1.8,
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
    let mut strongest = 0.0_f32;
    let mut second = 0.0_f32;
    let mut coverage = 0.0_f32;

    for cell_z in (base_cell_z - 2)..=(base_cell_z + 2) {
        for cell_x in (base_cell_x - 2)..=(base_cell_x + 2) {
            let coord = AtlasCoord::new(cell_x, cell_z);
            let Some(cell) = guides.cells().get(coord).copied() else {
                continue;
            };
            let Some(lobe) = macro_lobe_descriptor(coord, cell, meso_span_blocks as f32) else {
                continue;
            };
            let delta_x = sample_x - lobe.center_x;
            let delta_z = sample_z - lobe.center_z;
            let along = delta_x * lobe.heading_x + delta_z * lobe.heading_z;
            let across = delta_x * -lobe.heading_z + delta_z * lobe.heading_x;
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
            if contribution > strongest {
                second = strongest;
                strongest = contribution;
            } else if contribution > second {
                second = contribution;
            }

            let mask = (footprint * (0.30 + cell.hilliness * 0.70)).clamp(0.0, 1.0);
            coverage = coverage.max(mask);
        }
    }

    if strongest <= f32::EPSILON {
        return HillClusterApplySample::default();
    }

    HillClusterApplySample {
        coverage,
        lobe_height_blocks: strongest + second * 0.52,
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

fn macro_lobe_descriptor(
    coord: AtlasCoord,
    cell: MesoGuideCell,
    meso_span_blocks: f32,
) -> Option<MacroLobeDescriptor> {
    if cell.hilliness < 0.18 || cell.hill_height < 2.2 {
        return None;
    }

    let angle = lobe_hash01(coord, cell, LOBE_ANGLE_SALT) * TAU;
    let heading_x = angle.cos();
    let heading_z = angle.sin();
    let center_x = coord.x as f32 * meso_span_blocks
        + meso_span_blocks * 0.5
        + lerp_f32(-meso_span_blocks * 0.16, meso_span_blocks * 0.16, lobe_hash01(coord, cell, LOBE_JITTER_X_SALT));
    let center_z = coord.z as f32 * meso_span_blocks
        + meso_span_blocks * 0.5
        + lerp_f32(-meso_span_blocks * 0.16, meso_span_blocks * 0.16, lobe_hash01(coord, cell, LOBE_JITTER_Z_SALT));
    let radius_scale =
        (0.76 + cell.hilliness * 0.24 + (cell.hill_height / 14.0).clamp(0.0, 0.28)).clamp(0.76, 1.28);
    let height_scale = (0.74 + cell.hilliness * 0.26).clamp(0.74, 1.10);

    Some(MacroLobeDescriptor {
        center_x,
        center_z,
        heading_x,
        heading_z,
        radius_x_blocks: lerp_f32(20.0, 38.0, lobe_hash01(coord, cell, LOBE_RADIUS_X_SALT)) * radius_scale,
        radius_z_blocks: lerp_f32(16.0, 30.0, lobe_hash01(coord, cell, LOBE_RADIUS_Z_SALT)) * radius_scale,
        height_blocks: cell.hill_height
            * lerp_f32(0.82, 1.12, lobe_hash01(coord, cell, LOBE_HEIGHT_SALT))
            * height_scale,
    })
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
            local_peak_count >= 2,
            "expected neighboring guide cells to resolve as multiple macro lobes, found {local_peak_count}"
        );
    }
}
