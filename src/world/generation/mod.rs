mod context;
mod noise;
mod profile;
mod profiles;
mod realize;
mod sampler;

pub const SEA_LEVEL_Y: i32 = 0;
pub const WORLD_FLOOR_Y: i32 = -256;

#[allow(dead_code)]
pub const FLAT_WORLD_SURFACE_Y: i32 = SEA_LEVEL_Y;

pub use realize::generate_chunk;

#[cfg(test)]
mod tests {
    use super::context::{ColumnAtlasSample, ColumnRealization, GenerationPalette};
    use super::profile::TerrainProfile;
    use super::profiles::surface_y_for_profile;
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
    fn surface_and_subsurface_are_stone_only() {
        let registry = test_registry();
        let palette = GenerationPalette::from_registry(&registry);
        let column = ColumnRealization {
            surface_y: 4,
            profile: TerrainProfile::Plain,
        };

        assert_eq!(block_for_world_y(4, column, palette), palette.stone);
        assert_eq!(block_for_world_y(3, column, palette), palette.stone);
        assert_eq!(block_for_world_y(-6, column, palette), palette.stone);
        assert_eq!(block_for_world_y(5, column, palette), BlockId::AIR);
    }

    #[test]
    fn ocean_columns_do_not_fill_water_in_stone_phase() {
        let registry = test_registry();
        let palette = GenerationPalette::from_registry(&registry);
        let column = ColumnRealization {
            surface_y: -18,
            profile: TerrainProfile::DeepOcean,
        };

        assert_eq!(block_for_world_y(-24, column, palette), palette.stone);
        assert_eq!(block_for_world_y(-18, column, palette), palette.stone);
        assert_eq!(block_for_world_y(-17, column, palette), BlockId::AIR);
        assert_eq!(block_for_world_y(0, column, palette), BlockId::AIR);
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
            riverine_factor: 0.16,
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
            riverine_factor: 0.08,
            alpine_factor: 0.02,
        };

        let a = surface_y_for_profile(17, 0, 0, sample, TerrainProfile::Plain);
        let b = surface_y_for_profile(17, 96, 64, sample, TerrainProfile::Plain);

        assert_ne!(a, b);
        assert!((a - b).abs() >= 2);
    }
}
