use crate::world::atlas::{AtlasCell, AtlasCoord, MesoGuideCell};

use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};
use super::super::{
    FeatureInstance, MesoFeatureKind, cell_center_jitter, ellipse_footprint, hash01, lerp_f32,
    smoothstep_range,
};

const STRENGTH_MIN_BLOCKS: f32 = 4.6;
const STRENGTH_MAX_BLOCKS: f32 = 10.8;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
