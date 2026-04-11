mod context;
mod noise;
mod profile;
mod probe;
mod profiles;
mod realize;
mod sampler;

pub const SEA_LEVEL_Y: i32 = 0;
pub const WORLD_FLOOR_Y: i32 = -256;

#[allow(dead_code)]
pub const FLAT_WORLD_SURFACE_Y: i32 = SEA_LEVEL_Y;

pub use context::ColumnAtlasSample;
pub use probe::{
    ChunkGenerationProbe, ChunkSurfaceLodGrid, ChunkSurfaceLodSample, ColumnGenerationProbe,
    TerrainProfileCounts, probe_chunk, probe_column, sample_chunk_surface_lod,
};
pub use profile::TerrainProfile;
pub use realize::generate_chunk;

#[cfg(test)]
mod tests {
    use super::context::{
        ColumnAtlasSample, ColumnFillProfile, ColumnRealization, GenerationPalette, RiverStage,
    };
    use super::profile::surface_profile_blend;
    use super::profile::TerrainProfile;
    use super::profiles::{surface_y_for_profile, surface_y_for_sample};
    use super::realize::block_for_world_y;
    use super::*;
    use crate::world::{BlockId, BlockRegistry, ChunkCoord, LocalBlockCoord, WorldMeta};

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
            water_top_y: None,
            fill_profile: ColumnFillProfile::SoilWithGrassTop,
        };

        assert_eq!(block_for_world_y(4, 0, 0, 7, column, palette), palette.grass);
        assert_eq!(block_for_world_y(3, 0, 0, 7, column, palette), palette.dirt);
        assert_eq!(block_for_world_y(-6, 0, 0, 7, column, palette), palette.stone);
        assert_eq!(block_for_world_y(5, 0, 0, 7, column, palette), BlockId::AIR);
    }

    #[test]
    fn ocean_profile_fills_water_up_to_sea_level() {
        let registry = test_registry();
        let palette = GenerationPalette::from_registry(&registry);
        let column = ColumnRealization {
            surface_y: -5,
            stone_ceiling_y: -12,
            water_top_y: Some(SEA_LEVEL_Y),
            fill_profile: ColumnFillProfile::DeepOcean,
        };

        assert_eq!(block_for_world_y(-12, 0, 0, 7, column, palette), palette.stone);
        assert_eq!(block_for_world_y(-5, 0, 0, 7, column, palette), palette.mud);
        assert_eq!(block_for_world_y(-4, 0, 0, 7, column, palette), palette.water);
        assert_eq!(block_for_world_y(0, 0, 0, 7, column, palette), palette.water);
        assert_eq!(block_for_world_y(1, 0, 0, 7, column, palette), BlockId::AIR);
    }

    #[test]
    fn river_stage_profiles_pick_expected_bed_materials() {
        let registry = test_registry();
        let palette = GenerationPalette::from_registry(&registry);

        let headwaters = ColumnRealization {
            surface_y: 6,
            stone_ceiling_y: 0,
            water_top_y: None,
            fill_profile: ColumnFillProfile::River(RiverStage::Headwaters),
        };
        let middle = ColumnRealization {
            surface_y: 6,
            stone_ceiling_y: 0,
            water_top_y: None,
            fill_profile: ColumnFillProfile::River(RiverStage::Middle),
        };
        let lower = ColumnRealization {
            surface_y: 6,
            stone_ceiling_y: 0,
            water_top_y: None,
            fill_profile: ColumnFillProfile::River(RiverStage::Lower),
        };

        assert_eq!(block_for_world_y(6, 0, 0, 7, headwaters, palette), palette.grass);
        assert_eq!(block_for_world_y(5, 0, 0, 7, headwaters, palette), palette.gravel);
        let middle_block = block_for_world_y(5, 3, 6, 7, middle, palette);
        assert!(middle_block == palette.gravel || middle_block == palette.sand);
        assert_eq!(block_for_world_y(6, 9, 12, 7, middle, palette), palette.grass);
        let lower_block = block_for_world_y(5, 9, 12, 7, lower, palette);
        assert!(lower_block == palette.mud || lower_block == palette.sand);
    }

    #[test]
    fn non_sand_non_snow_land_surfaces_use_grass_top() {
        let registry = test_registry();
        let palette = GenerationPalette::from_registry(&registry);
        let headwaters = ColumnRealization {
            surface_y: 6,
            stone_ceiling_y: 0,
            water_top_y: None,
            fill_profile: ColumnFillProfile::River(RiverStage::Headwaters),
        };
        let soil = ColumnRealization {
            surface_y: 12,
            stone_ceiling_y: 3,
            water_top_y: None,
            fill_profile: ColumnFillProfile::SoilWithGrassTop,
        };

        assert_eq!(block_for_world_y(6, 0, 0, 7, headwaters, palette), palette.grass);
        assert_eq!(block_for_world_y(12, 0, 0, 7, soil, palette), palette.grass);
    }

    #[test]
    fn low_coastal_plain_can_resolve_to_coast_fill_profile() {
        let sample = ColumnAtlasSample {
            landness: 0.57,
            ocean_distance: 0.08,
            coast_factor: 0.46,
            continent_core_factor: 0.10,
            macro_elevation: 0.14,
            ridge_factor: 0.10,
            mountain_mass: 0.18,
            ruggedness: 0.14,
            river_source_potential: 0.08,
            river_flow_potential: 0.16,
            riverine_factor: 0.04,
            lake_potential: 0.02,
            temperature: 0.54,
            humidity: 0.48,
            aridity: 0.24,
            wetness: 0.20,
            polar_factor: 0.02,
            alpine_factor: 0.02,
        };

        let fill = super::realize::classify_fill_profile(sample, 2, 0.53, TerrainProfile::Plain);
        assert_eq!(fill, ColumnFillProfile::Coast);
    }

    #[test]
    fn coast_fill_profile_wins_over_river_signal_near_shore() {
        let sample = ColumnAtlasSample {
            landness: 0.60,
            ocean_distance: 0.10,
            coast_factor: 0.64,
            continent_core_factor: 0.08,
            macro_elevation: 0.12,
            ridge_factor: 0.06,
            mountain_mass: 0.10,
            ruggedness: 0.16,
            river_source_potential: 0.22,
            river_flow_potential: 0.48,
            riverine_factor: 0.82,
            lake_potential: 0.16,
            temperature: 0.56,
            humidity: 0.52,
            aridity: 0.22,
            wetness: 0.30,
            polar_factor: 0.02,
            alpine_factor: 0.02,
        };

        let fill = super::realize::classify_fill_profile(sample, 3, 0.53, TerrainProfile::Coast);
        assert_eq!(fill, ColumnFillProfile::Coast);
    }

    #[test]
    fn submerged_river_surface_uses_bed_material_and_water() {
        let registry = test_registry();
        let palette = GenerationPalette::from_registry(&registry);
        let column = ColumnRealization {
            surface_y: 6,
            stone_ceiling_y: 0,
            water_top_y: Some(7),
            fill_profile: ColumnFillProfile::River(RiverStage::Headwaters),
        };

        assert_eq!(block_for_world_y(6, 0, 0, 7, column, palette), palette.gravel);
        assert_eq!(block_for_world_y(7, 0, 0, 7, column, palette), palette.water);
    }

    #[test]
    fn profile_surfaces_span_ocean_to_ridge() {
        let sample = ColumnAtlasSample {
            landness: 0.68,
            ocean_distance: 0.30,
            coast_factor: 0.18,
            continent_core_factor: 0.46,
            macro_elevation: 0.54,
            ridge_factor: 0.58,
            mountain_mass: 0.62,
            ruggedness: 0.48,
            river_source_potential: 0.20,
            river_flow_potential: 0.32,
            riverine_factor: 0.16,
            lake_potential: 0.08,
            temperature: 0.48,
            humidity: 0.52,
            aridity: 0.24,
            wetness: 0.34,
            polar_factor: 0.06,
            alpine_factor: 0.26,
        };

        let deep_ocean = surface_y_for_profile(
            11,
            0,
            0,
            ColumnAtlasSample {
                landness: 0.18,
                ocean_distance: 0.82,
                ..sample
            },
            TerrainProfile::DeepOcean,
        );
        let shelf = surface_y_for_profile(
            11,
            0,
            0,
            ColumnAtlasSample {
                landness: 0.34,
                ocean_distance: 0.18,
                coast_factor: 0.52,
                ..sample
            },
            TerrainProfile::Shelf,
        );
        let coast = surface_y_for_profile(
            11,
            0,
            0,
            ColumnAtlasSample {
                coast_factor: 0.72,
                macro_elevation: 0.18,
                continent_core_factor: 0.12,
                mountain_mass: 0.10,
                ridge_factor: 0.08,
                ruggedness: 0.14,
                ..sample
            },
            TerrainProfile::Coast,
        );
        let plain = surface_y_for_profile(
            11,
            0,
            0,
            ColumnAtlasSample {
                macro_elevation: 0.24,
                ridge_factor: 0.12,
                mountain_mass: 0.10,
                ruggedness: 0.18,
                coast_factor: 0.24,
                ..sample
            },
            TerrainProfile::Plain,
        );
        let upland = surface_y_for_profile(
            11,
            0,
            0,
            ColumnAtlasSample {
                macro_elevation: 0.48,
                ridge_factor: 0.34,
                mountain_mass: 0.36,
                ruggedness: 0.42,
                coast_factor: 0.10,
                ..sample
            },
            TerrainProfile::Upland,
        );
        let ridge = surface_y_for_profile(11, 0, 0, sample, TerrainProfile::Ridge);

        assert!(deep_ocean < shelf);
        assert!(shelf < coast);
        assert!(coast < plain);
        assert!(plain < upland);
        assert!(upland < ridge);
    }

    #[test]
    fn plain_profile_uses_local_relief_noise() {
        let sample = ColumnAtlasSample {
            landness: 0.72,
            ocean_distance: 0.22,
            coast_factor: 0.16,
            continent_core_factor: 0.38,
            macro_elevation: 0.34,
            ridge_factor: 0.18,
            mountain_mass: 0.12,
            ruggedness: 0.22,
            river_source_potential: 0.12,
            river_flow_potential: 0.18,
            riverine_factor: 0.08,
            lake_potential: 0.04,
            temperature: 0.48,
            humidity: 0.54,
            aridity: 0.20,
            wetness: 0.26,
            polar_factor: 0.04,
            alpine_factor: 0.02,
        };

        let a = surface_y_for_profile(17, 0, 0, sample, TerrainProfile::Plain);
        let b = surface_y_for_profile(17, 96, 64, sample, TerrainProfile::Plain);

        assert_ne!(a, b);
        assert!((a - b).abs() >= 2);
    }

    #[test]
    fn chunk_probe_summarizes_profiles_and_surface_range() {
        let meta = WorldMeta::new(42);
        let probe = probe_chunk(ChunkCoord(5, 0, -8), &meta);

        assert_eq!(probe.profile_counts.total(), 1024);
        assert!(probe.surface_min_y <= probe.surface_max_y);
    }

    #[test]
    fn chunk_surface_lod_sampling_returns_even_grid() {
        let meta = WorldMeta::new(42);
        let grid = sample_chunk_surface_lod(ChunkCoord(5, 0, -8), 4, &meta);

        assert_eq!(grid.samples_per_axis, 8);
        assert_eq!(grid.samples.len(), 64);
        assert!(grid.samples.iter().all(|sample| sample.span_blocks == 4));
    }

    #[test]
    fn resolve_profile_promotes_ridge_for_strong_mountain_signal() {
        let sample = ColumnAtlasSample {
            landness: 0.78,
            ocean_distance: 1.0,
            coast_factor: 0.02,
            continent_core_factor: 0.88,
            macro_elevation: 0.72,
            ridge_factor: 0.52,
            mountain_mass: 0.44,
            ruggedness: 0.40,
            river_source_potential: 0.20,
            river_flow_potential: 0.22,
            riverine_factor: 0.10,
            lake_potential: 0.04,
            temperature: 0.32,
            humidity: 0.40,
            aridity: 0.22,
            wetness: 0.18,
            polar_factor: 0.18,
            alpine_factor: 0.18,
        };

        let profile = super::profile::resolve_profile(sample, 0.53);
        assert_eq!(profile, TerrainProfile::Ridge);
    }

    #[test]
    fn surface_profile_blend_normalizes_and_uses_multiple_weights_near_boundaries() {
        let sample = ColumnAtlasSample {
            landness: 0.55,
            ocean_distance: 0.22,
            coast_factor: 0.36,
            continent_core_factor: 0.22,
            macro_elevation: 0.31,
            ridge_factor: 0.20,
            mountain_mass: 0.24,
            ruggedness: 0.24,
            river_source_potential: 0.10,
            river_flow_potential: 0.22,
            riverine_factor: 0.18,
            lake_potential: 0.08,
            temperature: 0.52,
            humidity: 0.48,
            aridity: 0.26,
            wetness: 0.28,
            polar_factor: 0.04,
            alpine_factor: 0.08,
        };

        let blend = surface_profile_blend(sample, 0.53);
        let total =
            blend.deep_ocean + blend.shelf + blend.coast + blend.plain + blend.upland + blend.ridge;

        assert!((total - 1.0).abs() < 0.001);
        assert!(blend.coast > 0.0);
        assert!(blend.plain > 0.0);
        assert!(blend.upland > 0.0 || blend.ridge > 0.0);
    }

    #[test]
    fn blended_surface_softens_plain_to_ridge_transition() {
        let ridgeish = ColumnAtlasSample {
            landness: 0.59,
            ocean_distance: 0.16,
            coast_factor: 0.06,
            continent_core_factor: 0.24,
            macro_elevation: 0.32,
            ridge_factor: 0.21,
            mountain_mass: 0.34,
            ruggedness: 0.30,
            river_source_potential: 0.16,
            river_flow_potential: 0.34,
            riverine_factor: 0.20,
            lake_potential: 0.04,
            temperature: 0.46,
            humidity: 0.48,
            aridity: 0.20,
            wetness: 0.24,
            polar_factor: 0.06,
            alpine_factor: 0.20,
        };
        let plainish = ColumnAtlasSample {
            ridge_factor: 0.12,
            mountain_mass: 0.18,
            ruggedness: 0.18,
            macro_elevation: 0.22,
            alpine_factor: 0.04,
            ..ridgeish
        };

        let ridge_surface = surface_y_for_sample(17, 0, 0, ridgeish, 0.53);
        let plain_surface = surface_y_for_sample(17, 32, 0, plainish, 0.53);

        assert!((ridge_surface - plain_surface).abs() <= 18);
        assert!(ridge_surface > plain_surface);
    }
}
