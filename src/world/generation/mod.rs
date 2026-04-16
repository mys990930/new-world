mod context;
pub mod legacy;
mod noise;
mod profile;
mod probe;
mod profiles;
mod realize;
mod sampler;
mod surface;
pub mod v2;

pub const SEA_LEVEL_Y: i32 = 0;
pub const WORLD_FLOOR_Y: i32 = -256;

#[allow(dead_code)]
pub const FLAT_WORLD_SURFACE_Y: i32 = SEA_LEVEL_Y;

pub use context::ColumnAtlasSample;
pub use legacy::{
    GENERATOR_LABEL as LEGACY_GENERATOR_LABEL, generate_chunk as generate_chunk_legacy,
    probe_chunk as probe_chunk_legacy, probe_column as probe_column_legacy,
    sample_chunk_surface_lod as sample_chunk_surface_lod_legacy,
};
pub use probe::{
    ChunkGenerationProbe, ChunkSurfaceLodGrid, ChunkSurfaceLodSample, ColumnGenerationProbe,
    TerrainProfileCounts, probe_chunk, probe_column, sample_chunk_surface_lod,
};
pub use profile::TerrainProfile;
pub use legacy::generate_chunk;
pub use v2::{
    GENERATOR_LABEL as V2_GENERATOR_LABEL, BaseHeightfieldPrototype, ChunkCorridorWindow,
    ChunkGenerationV2Inputs, ChunkGenerationV2Scaffold, HydrologySolve,
    MesoAppliedPrototype, PrototypeColumn, RiverCorridorConstraint, SmoothedPrototype,
    V2ScaffoldStage, VoxelizationPlan, build_chunk_v2_scaffold,
    default_voxelization_plan, empty_base_heightfield_prototype,
    empty_chunk_corridor_window, empty_hydrology_solve, empty_meso_applied_prototype,
    empty_smoothed_prototype, prepare_chunk_v2_inputs,
};

#[cfg(test)]
mod tests {
    use super::context::{
        ColumnAtlasSample, ColumnFillProfile, ColumnRealization, GenerationPalette, RiverStage,
    };
    use super::profile::surface_profile_blend;
    use super::profile::TerrainProfile;
    use super::profiles::{surface_y_for_profile, surface_y_for_sample};
    use super::realize::{ColumnBlocks, block_for_world_y};
    use super::sampler::{
        generate_chunk_atlas_fields, generate_chunk_atlas_structure, generate_chunk_meso_guides,
    };
    use super::surface::{PreparedStructureGuide, build_chunk_surface_field};
    use super::*;
    use crate::world::{
        AtlasStructureMap, AtlasStructureRegionCoord, BlockId, BlockRegistry, ChunkCoord,
        DrainageNode, DrainageNodeKind, LocalBlockCoord, MountainChainId, MountainChainScale,
        MountainSpineSegment, RiverPathId, RiverPathKind, RiverPathSegment, WorldMeta,
    };

    fn test_registry() -> BlockRegistry {
        BlockRegistry::load_default().expect("default registry should load")
    }

    #[test]
    fn legacy_alias_matches_default_generate_chunk() {
        let meta = WorldMeta::new(7);
        let registry = test_registry();

        let default_chunk = generate_chunk(ChunkCoord(0, 0, 0), &meta, &registry);
        let legacy_chunk = generate_chunk_legacy(ChunkCoord(0, 0, 0), &meta, &registry);

        assert_eq!(default_chunk, legacy_chunk);
    }

    #[test]
    fn v2_scaffold_is_deterministic() {
        let meta = WorldMeta::new(42);

        let a = build_chunk_v2_scaffold(ChunkCoord(4, 0, -3), &meta);
        let b = build_chunk_v2_scaffold(ChunkCoord(4, 0, -3), &meta);

        assert_eq!(a, b);
        assert_eq!(a.stage, V2ScaffoldStage::RegionClassificationReady);
    }

    fn blocks(surface_block: BlockId, fill_block: BlockId) -> ColumnBlocks {
        ColumnBlocks {
            surface_block,
            fill_block,
        }
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

        assert_eq!(
            block_for_world_y(4, column, blocks(palette.grass, palette.dirt), palette),
            palette.grass
        );
        assert_eq!(
            block_for_world_y(3, column, blocks(palette.grass, palette.dirt), palette),
            palette.dirt
        );
        assert_eq!(
            block_for_world_y(2, column, blocks(palette.grass, palette.dirt), palette),
            palette.dirt
        );
        assert_eq!(
            block_for_world_y(-6, column, blocks(palette.grass, palette.dirt), palette),
            palette.stone
        );
        assert_eq!(
            block_for_world_y(5, column, blocks(palette.grass, palette.dirt), palette),
            BlockId::AIR
        );
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

        let column_blocks = blocks(palette.mud, palette.mud);
        assert_eq!(block_for_world_y(-12, column, column_blocks, palette), palette.stone);
        assert_eq!(block_for_world_y(-5, column, column_blocks, palette), palette.mud);
        assert_eq!(block_for_world_y(-4, column, column_blocks, palette), palette.water);
        assert_eq!(block_for_world_y(0, column, column_blocks, palette), palette.water);
        assert_eq!(block_for_world_y(1, column, column_blocks, palette), BlockId::AIR);
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

        assert_eq!(
            block_for_world_y(6, headwaters, blocks(palette.gravel, palette.gravel), palette),
            palette.gravel
        );
        assert_eq!(
            block_for_world_y(5, headwaters, blocks(palette.gravel, palette.gravel), palette),
            palette.gravel
        );
        let middle_block =
            block_for_world_y(5, middle, blocks(palette.sand, palette.sand), palette);
        assert!(middle_block == palette.gravel || middle_block == palette.sand);
        let middle_surface =
            block_for_world_y(6, middle, blocks(palette.gravel, palette.gravel), palette);
        assert!(middle_surface == palette.gravel || middle_surface == palette.sand);
        let lower_block =
            block_for_world_y(5, lower, blocks(palette.mud, palette.mud), palette);
        assert!(lower_block == palette.mud || lower_block == palette.sand);
        let lower_surface =
            block_for_world_y(6, lower, blocks(palette.sand, palette.sand), palette);
        assert!(lower_surface == palette.mud || lower_surface == palette.sand);
    }

    #[test]
    fn soil_land_surfaces_use_grass_top() {
        let registry = test_registry();
        let palette = GenerationPalette::from_registry(&registry);
        let soil = ColumnRealization {
            surface_y: 12,
            stone_ceiling_y: 3,
            water_top_y: None,
            fill_profile: ColumnFillProfile::SoilWithGrassTop,
        };

        assert_eq!(
            block_for_world_y(12, soil, blocks(palette.grass, palette.dirt), palette),
            palette.grass
        );
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

        let fill = super::realize::classify_fill_profile(
            7,
            0,
            0,
            sample,
            2,
            0.53,
            TerrainProfile::Plain,
            PreparedStructureGuide::default(),
        );
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

        let fill = super::realize::classify_fill_profile(
            7,
            0,
            0,
            sample,
            3,
            0.53,
            TerrainProfile::Coast,
            PreparedStructureGuide::default(),
        );
        assert_eq!(fill, ColumnFillProfile::Coast);
    }

    #[test]
    fn warm_alpine_land_does_not_resolve_to_frozen() {
        let sample = ColumnAtlasSample {
            landness: 0.66,
            ocean_distance: 0.28,
            coast_factor: 0.06,
            continent_core_factor: 0.34,
            macro_elevation: 0.52,
            ridge_factor: 0.28,
            mountain_mass: 0.92,
            ruggedness: 0.30,
            river_source_potential: 0.14,
            river_flow_potential: 0.10,
            riverine_factor: 0.08,
            lake_potential: 0.02,
            temperature: 0.48,
            humidity: 0.44,
            aridity: 0.20,
            wetness: 0.24,
            polar_factor: 0.08,
            alpine_factor: 0.74,
        };

        let fill = super::realize::classify_fill_profile(
            7,
            0,
            0,
            sample,
            18,
            0.53,
            TerrainProfile::Ridge,
            PreparedStructureGuide::default(),
        );
        assert_eq!(fill, ColumnFillProfile::SoilWithGrassTop);
    }

    #[test]
    fn cold_alpine_land_resolves_to_frozen() {
        let sample = ColumnAtlasSample {
            landness: 0.66,
            ocean_distance: 0.28,
            coast_factor: 0.06,
            continent_core_factor: 0.34,
            macro_elevation: 0.58,
            ridge_factor: 0.32,
            mountain_mass: 0.95,
            ruggedness: 0.34,
            river_source_potential: 0.16,
            river_flow_potential: 0.12,
            riverine_factor: 0.08,
            lake_potential: 0.02,
            temperature: 0.24,
            humidity: 0.42,
            aridity: 0.18,
            wetness: 0.22,
            polar_factor: 0.16,
            alpine_factor: 0.78,
        };

        let fill = super::realize::classify_fill_profile(
            7,
            0,
            0,
            sample,
            22,
            0.53,
            TerrainProfile::Ridge,
            PreparedStructureGuide::default(),
        );
        assert_eq!(fill, ColumnFillProfile::Frozen);
    }

    #[test]
    fn polar_land_resolves_to_frozen_even_without_alpine() {
        let sample = ColumnAtlasSample {
            landness: 0.62,
            ocean_distance: 0.22,
            coast_factor: 0.04,
            continent_core_factor: 0.28,
            macro_elevation: 0.30,
            ridge_factor: 0.10,
            mountain_mass: 0.12,
            ruggedness: 0.18,
            river_source_potential: 0.08,
            river_flow_potential: 0.06,
            riverine_factor: 0.04,
            lake_potential: 0.02,
            temperature: 0.10,
            humidity: 0.36,
            aridity: 0.12,
            wetness: 0.16,
            polar_factor: 0.60,
            alpine_factor: 0.14,
        };

        let fill = super::realize::classify_fill_profile(
            7,
            0,
            0,
            sample,
            10,
            0.53,
            TerrainProfile::Plain,
            PreparedStructureGuide::default(),
        );
        assert_eq!(fill, ColumnFillProfile::Frozen);
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

        let column_blocks = blocks(palette.gravel, palette.gravel);
        assert_eq!(block_for_world_y(6, column, column_blocks, palette), palette.gravel);
        assert_eq!(block_for_world_y(7, column, column_blocks, palette), palette.water);
    }

    #[test]
    fn coast_surface_stays_at_or_above_sea_level() {
        let sample = ColumnAtlasSample {
            landness: 0.58,
            ocean_distance: 0.06,
            coast_factor: 0.70,
            continent_core_factor: 0.08,
            macro_elevation: 0.10,
            ridge_factor: 0.04,
            mountain_mass: 0.08,
            ruggedness: 0.12,
            river_source_potential: 0.04,
            river_flow_potential: 0.10,
            riverine_factor: 0.40,
            lake_potential: 0.06,
            temperature: 0.56,
            humidity: 0.50,
            aridity: 0.24,
            wetness: 0.22,
            polar_factor: 0.02,
            alpine_factor: 0.02,
        };

        let height = super::profiles::surface_height_for_profile(7, 0, 0, sample, TerrainProfile::Coast);
        assert!(height >= 0.0);
    }

    #[test]
    fn coast_profile_does_not_fill_sea_water_by_default() {
        let registry = test_registry();
        let palette = GenerationPalette::from_registry(&registry);
        let column = ColumnRealization {
            surface_y: 0,
            stone_ceiling_y: -8,
            water_top_y: None,
            fill_profile: ColumnFillProfile::Coast,
        };

        let column_blocks = blocks(palette.sand, palette.sand);
        assert_eq!(block_for_world_y(0, column, column_blocks, palette), palette.sand);
        assert_eq!(block_for_world_y(1, column, column_blocks, palette), BlockId::AIR);
    }

    #[test]
    fn emergent_shelf_prefers_coast_fill_over_dry_shallow_ocean() {
        let sample = ColumnAtlasSample {
            landness: 0.528,
            ocean_distance: 0.0,
            coast_factor: 0.0,
            continent_core_factor: 0.0,
            macro_elevation: 0.042,
            ridge_factor: 0.154,
            mountain_mass: 0.685,
            ruggedness: 0.0,
            river_source_potential: 0.0,
            river_flow_potential: 0.0,
            riverine_factor: 0.0,
            lake_potential: 0.0,
            temperature: 0.5,
            humidity: 0.5,
            aridity: 0.2,
            wetness: 0.2,
            polar_factor: 0.0,
            alpine_factor: 0.55,
        };

        let fill = super::realize::classify_fill_profile(
            42,
            5136,
            -4592,
            sample,
            16,
            0.53,
            TerrainProfile::Shelf,
            PreparedStructureGuide::default(),
        );

        assert_eq!(fill, ColumnFillProfile::Coast);
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
    fn smoothed_surface_field_reduces_local_height_jitter() {
        let meta = WorldMeta::new(42);
        let coord = ChunkCoord(5, 0, -8);
        let atlas_fields = generate_chunk_atlas_fields(coord, &meta);
        let atlas_structure = generate_chunk_atlas_structure(coord, &meta);
        let atlas_meso = generate_chunk_meso_guides(coord, &meta, &atlas_fields, &atlas_structure);
        let surface_field =
            build_chunk_surface_field(coord, &meta, &atlas_fields, &atlas_structure, &atlas_meso);
        let mut raw_delta_sum = 0.0_f32;
        let mut smooth_delta_sum = 0.0_f32;
        let mut samples = 0_u32;

        for local_z in 0..31_u8 {
            for local_x in 0..31_u8 {
                let center = surface_field.column(local_x, local_z);
                let east = surface_field.column(local_x + 1, local_z);
                let south = surface_field.column(local_x, local_z + 1);

                raw_delta_sum += (center.raw_surface_y - east.raw_surface_y).abs();
                raw_delta_sum += (center.raw_surface_y - south.raw_surface_y).abs();
                smooth_delta_sum += (center.surface_y - east.surface_y).abs();
                smooth_delta_sum += (center.surface_y - south.surface_y).abs();
                samples += 2;
            }
        }

        assert!(samples > 0);
        assert!(smooth_delta_sum < raw_delta_sum);
    }

    #[test]
    fn manual_structure_segments_rasterize_into_surface_guides() {
        let meta = WorldMeta::new(42);
        let coord = ChunkCoord(8, 0, 8);
        let atlas_fields = generate_chunk_atlas_fields(coord, &meta);
        let mut atlas_structure = AtlasStructureMap::empty(atlas_fields.area());
        atlas_structure
            .mountain_chains_mut()
            .push_segment(MountainSpineSegment {
                chain_id: MountainChainId(1),
                owner_region: AtlasStructureRegionCoord::new(0, 0),
                branch_order: 0,
                scale: MountainChainScale::Major,
                start: crate::world::AtlasCoord::new(0, 0),
                end: crate::world::AtlasCoord::new(1, 0),
                strength: 1.0,
                half_width_cells: 2.0,
            });
        atlas_structure
            .drainage_mut()
            .push_segment(RiverPathSegment {
                river_id: RiverPathId(7),
                owner_region: AtlasStructureRegionCoord::new(0, 0),
                kind: RiverPathKind::Trunk,
                order: 2,
                start: crate::world::AtlasCoord::new(0, 0),
                end: crate::world::AtlasCoord::new(1, 0),
                bankfull_width_cells: 2.0,
                downstream_cells_start: 0.0,
                downstream_cells_end: 1.0,
            });
        atlas_structure
            .drainage_mut()
            .push_node(DrainageNode {
                coord: crate::world::AtlasCoord::new(0, 0),
                kind: DrainageNodeKind::Confluence,
                river_id: RiverPathId(7),
                order: 2,
            });

        let atlas_meso = generate_chunk_meso_guides(coord, &meta, &atlas_fields, &atlas_structure);
        let surface_field =
            build_chunk_surface_field(coord, &meta, &atlas_fields, &atlas_structure, &atlas_meso);
        let near_origin = surface_field.column(0, 0);
        let later = surface_field.column(31, 0);

        assert!(near_origin.structure.ridge_weight > 0.0);
        assert!(near_origin.structure.channel_weight > 0.0);
        assert_eq!(near_origin.structure.channel_order, 2);
        assert!(near_origin.structure.confluence_weight > 0.0);
        assert!(later.structure.along_channel_cells > near_origin.structure.along_channel_cells);
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
