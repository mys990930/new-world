use crate::world::WorldBlockCoord;

use super::surface_plan::{SurfaceHydrologyRole, SurfacePlanArea};

#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceFeaturePlan {
    pub placements: Vec<SurfaceFeaturePlacement>,
    pub stats: SurfaceFeaturePlanStats,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SurfaceFeaturePlanStats {
    pub candidate_column_count: usize,
    pub placement_count: usize,
    pub skipped_water_column_count: usize,
    pub skipped_non_target_surface_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceFeaturePlacement {
    pub anchor: WorldBlockCoord,
    pub feature: SurfaceFeatureRef,
    pub footprint_radius_blocks: u8,
    pub placement_seed: u64,
    pub quarter_turns: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceFeatureRef {
    Prop { prop_key: &'static str },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RockFeaturePlacementConfig {
    pub seed: u64,
    pub density_percent: u8,
    pub prop_keys: &'static [&'static str],
}

impl Default for RockFeaturePlacementConfig {
    fn default() -> Self {
        Self {
            seed: 0,
            density_percent: 4,
            prop_keys: &[
                "rock_pebble_1x1x1",
                "rock_flat_2x2x1",
                "rock_stacked_2x2x1_plus_1",
                "rock_low_cluster_4x3",
            ],
        }
    }
}

pub fn generate_rock_feature_placements(
    area: &SurfacePlanArea,
    config: RockFeaturePlacementConfig,
) -> SurfaceFeaturePlan {
    let mut placements = Vec::new();
    let mut stats = SurfaceFeaturePlanStats::default();
    let density = config.density_percent.min(100);
    if density == 0 || config.prop_keys.is_empty() {
        return SurfaceFeaturePlan { placements, stats };
    }

    for column in &area.columns {
        if column.water_y.is_some() {
            stats.skipped_water_column_count = stats.skipped_water_column_count.saturating_add(1);
            continue;
        }
        if !is_sandy_or_riverside_rock_surface(
            column.hydrology_role,
            column.top_block,
            column.sediment_block,
        ) {
            stats.skipped_non_target_surface_count =
                stats.skipped_non_target_surface_count.saturating_add(1);
            continue;
        }
        stats.candidate_column_count = stats.candidate_column_count.saturating_add(1);

        let placement_seed = rock_placement_seed(config.seed, column.world_x, column.world_z);
        if (placement_seed % 100) as u8 >= density {
            continue;
        }

        let prop_index = (placement_seed as usize >> 8) % config.prop_keys.len();
        placements.push(SurfaceFeaturePlacement {
            anchor: WorldBlockCoord(
                column.world_x,
                column.surface_y.saturating_add(1),
                column.world_z,
            ),
            feature: SurfaceFeatureRef::Prop {
                prop_key: config.prop_keys[prop_index],
            },
            footprint_radius_blocks: 1,
            placement_seed,
            quarter_turns: ((placement_seed >> 24) & 0b11) as u8,
        });
    }

    stats.placement_count = placements.len();
    SurfaceFeaturePlan { placements, stats }
}

fn is_sandy_or_riverside_rock_surface(
    role: SurfaceHydrologyRole,
    top_block: &'static str,
    sediment_block: &'static str,
) -> bool {
    match role {
        SurfaceHydrologyRole::Coast => matches!(top_block, "sand" | "wet_sand" | "gravel"),
        SurfaceHydrologyRole::River => {
            matches!(
                top_block,
                "sand" | "wet_sand" | "gravel" | "wet_gravel" | "silt"
            ) || matches!(sediment_block, "gravel" | "silt")
        }
        SurfaceHydrologyRole::Land | SurfaceHydrologyRole::DryBasin => {
            matches!(top_block, "sand" | "wet_sand" | "red_sand" | "gravel")
        }
        SurfaceHydrologyRole::Ocean
        | SurfaceHydrologyRole::Lake
        | SurfaceHydrologyRole::Wetland
        | SurfaceHydrologyRole::Ridge => false,
    }
}

fn rock_placement_seed(seed: u64, world_x: i32, world_z: i32) -> u64 {
    let mut value = seed
        ^ (world_x as i64 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (world_z as i64 as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = value.wrapping_add(0x94d0_49bb_1331_11eb);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generation::surface_plan::{
        SurfaceColumnPlan, SurfaceHydrologyRole, SurfacePlanArea, SurfacePlanAreaStats,
        SurfacePlanConfig,
    };

    #[test]
    fn rock_feature_placements_are_deterministic_prop_refs() {
        let area = test_surface_area();
        let config = RockFeaturePlacementConfig {
            seed: 42,
            density_percent: 100,
            prop_keys: &["rock_pebble_1x1x1", "rock_flat_2x2x1"],
        };

        let a = generate_rock_feature_placements(&area, config.clone());
        let b = generate_rock_feature_placements(&area, config);

        assert_eq!(a, b);
        assert_eq!(a.stats.candidate_column_count, 2);
        assert_eq!(a.stats.skipped_water_column_count, 1);
        assert_eq!(a.stats.skipped_non_target_surface_count, 1);
        assert_eq!(a.placements.len(), 2);
        assert!(a.placements.iter().all(|placement| {
            matches!(placement.feature, SurfaceFeatureRef::Prop { .. }) && placement.anchor.1 == 11
        }));
    }

    fn test_surface_area() -> SurfacePlanArea {
        SurfacePlanArea {
            width: 2,
            height: 2,
            sample_spacing_blocks: 1.0,
            columns: vec![
                test_column(0, 0, None, SurfaceHydrologyRole::Coast, "sand", "silt"),
                test_column(1, 0, Some(12), SurfaceHydrologyRole::Coast, "sand", "silt"),
                test_column(0, 1, None, SurfaceHydrologyRole::Land, "grass", "gravel"),
                test_column(
                    1,
                    1,
                    None,
                    SurfaceHydrologyRole::River,
                    "wet_gravel",
                    "gravel",
                ),
            ],
            stats: SurfacePlanAreaStats::default(),
            config: SurfacePlanConfig::default(),
        }
    }

    fn test_column(
        world_x: i32,
        world_z: i32,
        water_y: Option<i32>,
        hydrology_role: SurfaceHydrologyRole,
        top_block: &'static str,
        sediment_block: &'static str,
    ) -> SurfaceColumnPlan {
        SurfaceColumnPlan {
            world_x,
            world_z,
            surface_y: 10,
            water_y,
            hydrology_role,
            top_block,
            subsurface_block: "dirt",
            base_block: "stone",
            underwater_top_block: "silt",
            exposed_block: "rock",
            sediment_block,
            soil_depth_blocks: 3,
            vegetation_allowed: true,
            cover_phase: 0,
        }
    }
}
