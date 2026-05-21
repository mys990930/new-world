use super::super::mapping::resolve_contour_band_height;
use super::super::river::{
    deterministic_river_bank_variation_blocks, deterministic_river_bed_variation_blocks,
};
use super::super::stats::neighbor_indices;
use super::*;
use crate::world::generation::graph::WorldPlanePoint;
use crate::world::generation::macro_field::{
    MacroFieldSample, MacroFieldTile, MacroFieldTileConfig, MacroFieldTileStats,
};
use crate::world::generation::macro_map::MacroSurfaceKind;
use crate::world::generation::{
    DEFAULT_HEIGHTFIELD_CONTOUR_STEP_BLOCKS, DEFAULT_HEIGHTFIELD_MAX_BLOCKS,
    DEFAULT_HEIGHTFIELD_MIN_BLOCKS, DEFAULT_HEIGHTFIELD_NORMALIZED_MAX,
    DEFAULT_HEIGHTFIELD_NORMALIZED_MIN, DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD,
    DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS, HeightfieldConfig, HeightfieldContourConfig,
    HeightfieldPerlinConfig, HeightfieldTerrainKind, HeightfieldTile, generate_heightfield_tile,
};

#[test]
fn heightfield_tile_generation_is_deterministic() {
    let macro_tile = test_macro_tile();
    let config = HeightfieldConfig::default();

    let first = generate_heightfield_tile(&macro_tile, config);
    let second = generate_heightfield_tile(&macro_tile, config);

    assert_eq!(first, second);
}

#[test]
fn dimensions_and_column_count_match_macro_tile() {
    let macro_tile = test_macro_tile();
    let tile = generate_heightfield_tile(&macro_tile, HeightfieldConfig::default());

    assert_eq!(tile.width, macro_tile.config.width);
    assert_eq!(tile.height, macro_tile.config.height);
    assert_eq!(tile.columns.len(), macro_tile.samples.len());
    assert_eq!(tile.stats.column_count, macro_tile.samples.len());
    assert!(tile.column(0, 0).is_some());
    assert!(tile.column(tile.width, 0).is_none());
}

#[test]
fn heights_are_finite_and_stats_are_ordered() {
    let tile = generate_heightfield_tile(&test_macro_tile(), HeightfieldConfig::default());

    assert!(tile.columns.iter().all(|column| {
        column.surface_height_blocks.is_finite()
            && column.raw_surface_height_blocks.is_finite()
            && column.contour_guided_surface_height_blocks.is_finite()
            && column.constrained_surface_height_blocks.is_finite()
            && column.meso_delta_blocks == 0.0
            && column.micro_relief_blocks == 0.0
    }));
    assert!(tile.stats.min_surface_height_blocks <= tile.stats.max_surface_height_blocks);
}

#[test]
fn column_heights_are_snapped_to_integer_blocks() {
    let sample = sample(0.0, 0.0, 0.123, 0.0, 0.0, 0.0, 0.0);
    let column = heightfield_column_from_sample(&sample, HeightfieldConfig::default());

    assert_eq!(column.surface_height_blocks.fract(), 0.0);
    assert_eq!(column.surface_height_blocks, column.surface_y as f32);
}

#[test]
fn default_contour_step_is_one_block_without_smoothing() {
    let contour = HeightfieldContourConfig::default();

    assert_eq!(contour.step_blocks, 1.0);
    assert_eq!(contour.min_gap_blocks, 0.0);
    assert_eq!(contour.river_min_gap_blocks, 0.0);
    assert_eq!(contour.band_smoothing, 0.0);
}

#[test]
fn default_perlin_is_disabled_and_keeps_micro_relief_zero() {
    let config = HeightfieldConfig::default();
    let column =
        heightfield_column_from_sample(&sample(17.0, 29.0, 0.25, 0.0, 0.0, 0.0, 0.0), config);

    assert!(!config.perlin.enabled);
    assert_eq!(column.micro_relief_blocks, 0.0);
}

#[test]
fn enabled_perlin_produces_bounded_land_micro_relief() {
    let config = HeightfieldConfig {
        perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
        ..HeightfieldConfig::default()
    };
    let column =
        heightfield_column_from_sample(&sample(17.0, 29.0, 0.25, 0.0, 0.0, 0.0, 0.0), config);

    assert_ne!(column.micro_relief_blocks, 0.0);
    assert!(
        column.micro_relief_blocks.abs() <= config.perlin.max_abs_blocks,
        "micro relief should stay bounded: {}",
        column.micro_relief_blocks
    );
}

#[test]
fn preview_perlin_applies_micro_relief_before_contour_band() {
    let config = HeightfieldConfig {
        perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
        ..HeightfieldConfig::default()
    };
    let column =
        heightfield_column_from_sample(&sample(17.0, 29.0, 0.25, 0.0, 0.0, 0.0, 0.0), config);
    let expected = resolve_contour_band_height(
        column.raw_surface_height_blocks + column.micro_relief_blocks,
        config.contour,
    );

    assert_eq!(
        config.perlin.placement,
        HeightfieldPerlinPlacement::BeforeContour
    );
    assert_eq!(column.contour_guided_surface_height_blocks, expected);
    assert_eq!(
        column.surface_height_blocks, expected,
        "preview Perlin should perturb the contour source rather than stack after the band"
    );
}

#[test]
fn enabled_perlin_keeps_micro_relief_zero_for_water_and_river_columns() {
    let config = HeightfieldConfig {
        perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
        ..HeightfieldConfig::default()
    };
    let ocean =
        heightfield_column_from_sample(&sample(17.0, 29.0, -0.2, 1.0, 0.0, 0.0, 0.0), config);
    let lake =
        heightfield_column_from_sample(&sample(17.0, 29.0, -0.2, 0.0, 1.0, 0.0, 0.0), config);
    let river = heightfield_column_from_sample(&sample_with_river(17.0, 29.0, 0.25, 0.75), config);

    assert_eq!(ocean.micro_relief_blocks, 0.0);
    assert_eq!(lake.micro_relief_blocks, 0.0);
    assert_eq!(river.micro_relief_blocks, 0.0);
}

#[test]
fn enabled_perlin_can_perturb_river_bed_without_moving_water_surface() {
    let mut sample = sample_with_river(37.0, -91.0, 0.25, 0.82);
    sample.river_core_strength = 1.0;
    sample.river_shoulder_strength = 1.0;
    sample.river_valley_strength = 1.0;
    sample.river_bed_depth_hint = 0.55;
    sample.river_bank_roughness_hint = 0.75;
    let disabled = heightfield_column_from_sample(&sample, HeightfieldConfig::default());
    let enabled_config = HeightfieldConfig {
        perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
        ..HeightfieldConfig::default()
    };
    let enabled = heightfield_column_from_sample(&sample, enabled_config);

    assert_ne!(
        perlin::river_bed_relief_blocks(&sample, enabled_config.perlin),
        0.0
    );
    assert_eq!(
        enabled.water_level_blocks, disabled.water_level_blocks,
        "river bed Perlin should not move the river water surface"
    );
    assert_eq!(
        enabled.water_level_blocks, disabled.water_level_blocks,
        "river bed Perlin should not move the river water surface"
    );
    assert_eq!(enabled.micro_relief_blocks, 0.0);
}

#[test]
fn enabled_perlin_can_perturb_river_bank_surface() {
    let mut sample = sample(91.0, -37.0, 0.18, 0.0, 0.0, 0.0, 0.0);
    sample.river_shoulder_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD * 0.70;
    sample.river_valley_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD * 0.70;
    sample.river_distance_blocks = 36.0;
    sample.river_flow_hint = 0.68;
    sample.river_bank_roughness_hint = 0.9;
    sample.river_gravel_hint = 0.65;
    let disabled = heightfield_column_from_sample(&sample, HeightfieldConfig::default());
    let enabled_config = HeightfieldConfig {
        perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
        ..HeightfieldConfig::default()
    };
    let enabled = heightfield_column_from_sample(&sample, enabled_config);

    assert_ne!(
        perlin::river_bank_relief_blocks(&sample, enabled_config.perlin),
        0.0
    );
    assert_eq!(enabled.water_level_blocks, None);
    assert_eq!(enabled.terrain_kind, HeightfieldTerrainKind::Land);
    assert_eq!(
        enabled.terrain_kind, disabled.terrain_kind,
        "river bank Perlin should not change terrain ownership"
    );
}

#[test]
fn enabled_perlin_can_perturb_ocean_bed_without_moving_water_surface() {
    let sample = sample(53.0, -79.0, -0.18, 1.0, 0.0, 0.0, 0.0);
    let disabled = heightfield_column_from_sample(&sample, HeightfieldConfig::default());
    let enabled_config = HeightfieldConfig {
        perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
        ..HeightfieldConfig::default()
    };
    let enabled = heightfield_column_from_sample(&sample, enabled_config);

    assert_ne!(
        perlin::ocean_bed_relief_blocks(&sample, enabled_config.perlin),
        0.0
    );
    assert_eq!(enabled.micro_relief_blocks, 0.0);
    assert_eq!(
        enabled.water_level_blocks, disabled.water_level_blocks,
        "ocean bed Perlin should not move the sea surface"
    );
    assert_ne!(
        enabled.constrained_surface_height_blocks, disabled.constrained_surface_height_blocks,
        "ocean bed Perlin should perturb terrain bed before snapping"
    );
}

#[test]
fn enabled_perlin_uses_land_micro_relief_for_ocean_owned_above_sea_terrain() {
    let sample = sample(37.0, -91.0, 0.18, 1.0, 0.0, 0.0, 0.0);
    let config = HeightfieldConfig {
        perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
        ..HeightfieldConfig::default()
    };
    let column = heightfield_column_from_sample(&sample, config);

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
    assert_eq!(column.water_level_blocks, None);
    assert_ne!(
        column.micro_relief_blocks, 0.0,
        "ocean-owned above-sea terrain should use the same micro relief map as land"
    );
}

#[test]
fn sea_level_ocean_owned_border_uses_land_micro_relief() {
    let sample = sample(53.0, -79.0, 0.0, 1.0, 0.0, 0.0, 0.0);
    let config = HeightfieldConfig {
        perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
        ..HeightfieldConfig::default()
    };
    let column = heightfield_column_from_sample(&sample, config);
    let mut land_sample = sample;
    land_sample.ocean_mask = 0.0;
    let land_column = heightfield_column_from_sample(&land_sample, config);

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
    assert_eq!(
        column.micro_relief_blocks, land_column.micro_relief_blocks,
        "exact sea-level ocean-owned border should reuse the ordinary land micro relief map"
    );
    assert_ne!(column.micro_relief_blocks, 0.0);
    assert_eq!(
        perlin::ocean_bed_relief_blocks(&sample, config.perlin),
        0.0,
        "ocean bed Perlin should not move an exact sea-level source column"
    );
}

#[test]
fn shallow_ocean_owned_border_band_uses_land_micro_relief() {
    let shallow_ocean = sample(53.0, -79.0, -0.003, 1.0, 0.0, 0.0, 0.0);
    let config = HeightfieldConfig {
        perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
        ..HeightfieldConfig::default()
    };
    let column = heightfield_column_from_sample(&shallow_ocean, config);
    let mut land_sample = shallow_ocean;
    land_sample.ocean_mask = 0.0;
    let land_column = heightfield_column_from_sample(&land_sample, config);
    let deep_ocean =
        heightfield_column_from_sample(&sample(53.0, -79.0, -0.02, 1.0, 0.0, 0.0, 0.0), config);

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
    assert_eq!(
        column.micro_relief_blocks, land_column.micro_relief_blocks,
        "shallow ocean-owned border band should reuse ordinary land micro relief"
    );
    assert_ne!(column.micro_relief_blocks, 0.0);
    assert_eq!(
        deep_ocean.micro_relief_blocks, 0.0,
        "deeper submerged ocean should stay protected from land micro relief"
    );
    assert_eq!(
        column.water_level_blocks,
        Some(DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS),
        "land micro relief must not directly move the sea-level water surface"
    );
}

#[test]
fn enabled_perlin_is_deterministic_for_same_world_position_and_config() {
    let config = HeightfieldConfig {
        perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
        ..HeightfieldConfig::default()
    };
    let sample = sample(17.0, 29.0, 0.25, 0.0, 0.0, 0.0, 0.0);
    let first = heightfield_column_from_sample(&sample, config);
    let second = heightfield_column_from_sample(&sample, config);

    assert_eq!(first.micro_relief_blocks, second.micro_relief_blocks);
    assert_eq!(first.surface_height_blocks, second.surface_height_blocks);
}

#[test]
fn contour_guided_height_is_pure_lower_band_and_snaps() {
    let config = HeightfieldConfig::default();
    let sample = sample(0.0, 0.0, 0.123, 0.0, 0.0, 0.0, 0.0);
    let column = heightfield_column_from_sample(&sample, config);
    let lower = (column.raw_surface_height_blocks / config.contour.step_blocks).floor()
        * config.contour.step_blocks;
    let expected = resolve_contour_band_height(column.raw_surface_height_blocks, config.contour);

    assert!(
        column.contour_guided_surface_height_blocks <= lower,
        "gap policy should not raise the lower contour band"
    );
    assert_eq!(column.contour_guided_surface_height_blocks, expected);
    assert_eq!(column.surface_height_blocks.fract(), 0.0);
}

#[test]
fn default_contour_snap_preserves_raw_block_scale() {
    let config = HeightfieldConfig::default();
    let below_one =
        heightfield_column_from_sample(&sample(0.0, 0.0, 0.00048, 0.0, 0.0, 0.0, 0.0), config);
    let at_one =
        heightfield_column_from_sample(&sample(0.0, 0.0, 0.00049, 0.0, 0.0, 0.0, 0.0), config);
    let below_two =
        heightfield_column_from_sample(&sample(0.0, 0.0, 0.00096, 0.0, 0.0, 0.0, 0.0), config);
    let at_two =
        heightfield_column_from_sample(&sample(0.0, 0.0, 0.00098, 0.0, 0.0, 0.0, 0.0), config);

    assert!(
        below_one.raw_surface_height_blocks < 1.0,
        "test input should sit just below the first block"
    );
    assert_eq!(
        below_one.surface_height_blocks, 0.0,
        "raw 0.0..0.999 should remain visible y=0"
    );
    assert_eq!(
        at_one.surface_height_blocks, 1.0,
        "raw 1.0..1.999 should become visible y=1"
    );
    assert_eq!(
        below_two.surface_height_blocks, 1.0,
        "default contour snap must not halve raw block scale"
    );
    assert_eq!(
        at_two.surface_height_blocks, 2.0,
        "raw 2.0..2.999 should become visible y=2"
    );
}

#[test]
fn default_river_corridor_uses_same_gap_but_can_cut_bed() {
    let config = HeightfieldConfig::default();
    let land =
        heightfield_column_from_sample(&sample(0.0, 0.0, 0.01000, 0.0, 0.0, 0.0, 0.0), config);
    let mut river_sample = sample_with_river(0.0, 0.0, 0.01000, 0.75);
    river_sample.river_bed_depth_hint = 1.0;
    let river = heightfield_column_from_sample(&river_sample, config);

    assert_eq!(
        config.contour.min_gap_blocks, config.contour.river_min_gap_blocks,
        "default launch slice uses the same zero-block gap for land and river corridors"
    );
    assert!(
        river.surface_height_blocks < land.surface_height_blocks,
        "river bed hint should cut the bed below the surrounding land: land={} river={}",
        land.surface_height_blocks,
        river.surface_height_blocks
    );
    assert!(
        river.water_level_blocks.is_some(),
        "river corridor should keep water separate from the carved bed"
    );
}

#[test]
fn river_corridor_gap_can_still_override_general_land_gap() {
    let config = HeightfieldConfig {
        contour: HeightfieldContourConfig {
            min_gap_blocks: 4.0,
            river_min_gap_blocks: 1.0,
            ..HeightfieldContourConfig::default()
        },
        ..HeightfieldConfig::default()
    };
    let land =
        heightfield_column_from_sample(&sample(0.0, 0.0, 0.00110, 0.0, 0.0, 0.0, 0.0), config);
    let river = heightfield_column_from_sample(&sample_with_river(0.0, 0.0, 0.00110, 0.75), config);

    assert_eq!(
        land.surface_height_blocks, 0.0,
        "wider configured land gap should still hold ordinary terrain back"
    );
    assert!(
        river.surface_height_blocks < land.surface_height_blocks,
        "river corridors keep the smaller gap and then apply river bed carve: land={} river={}",
        land.surface_height_blocks,
        river.surface_height_blocks
    );
}

#[test]
fn ordinary_land_visible_surface_preserves_raw_neighbor_jump() {
    let config = MacroFieldTileConfig::new(0.0, 0.0, 4, 1, 1.0);
    let samples = vec![
        sample(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        sample(1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0),
        sample(2.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0),
        sample(3.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0),
    ];
    let macro_tile = MacroFieldTile {
        config,
        samples,
        stats: MacroFieldTileStats::default(),
    };
    let tile = generate_heightfield_tile(&macro_tile, HeightfieldConfig::default());
    let heights = tile
        .columns
        .iter()
        .map(|column| column.surface_height_blocks)
        .collect::<Vec<_>>();

    assert_eq!(
        tile.column(1, 0)
            .expect("steep land")
            .raw_surface_height_blocks,
        DEFAULT_HEIGHTFIELD_MAX_BLOCKS
    );
    assert_eq!(
        heights,
        vec![
            0.0,
            DEFAULT_HEIGHTFIELD_MAX_BLOCKS,
            DEFAULT_HEIGHTFIELD_MAX_BLOCKS,
            DEFAULT_HEIGHTFIELD_MAX_BLOCKS
        ]
    );
    assert!(
        tile.stats.max_visible_neighbor_delta_blocks > 1.0,
        "ordinary land should preserve raw-scale jumps when the source field jumps"
    );
}

#[test]
fn launch_relief_scale_uses_experimental_large_block_domain() {
    let high = heightfield_column_from_sample(
        &sample(
            0.0,
            0.0,
            DEFAULT_HEIGHTFIELD_NORMALIZED_MAX,
            0.0,
            0.0,
            0.0,
            0.0,
        ),
        HeightfieldConfig::default(),
    );

    assert_eq!(DEFAULT_HEIGHTFIELD_NORMALIZED_MIN, -0.5);
    assert_eq!(DEFAULT_HEIGHTFIELD_NORMALIZED_MAX, 1.0);
    assert_eq!(DEFAULT_HEIGHTFIELD_MAX_BLOCKS, 2048.0);
    assert_eq!(DEFAULT_HEIGHTFIELD_MIN_BLOCKS, -1024.0);
    assert_eq!(
        high.raw_surface_height_blocks, DEFAULT_HEIGHTFIELD_MAX_BLOCKS,
        "macro relief should use the experimental heightfield block-domain resolution"
    );
}

#[test]
fn experimental_interest_range_maps_to_large_block_span() {
    let config = HeightfieldConfig::default();
    let low = heightfield_column_from_sample(&sample(0.0, 0.0, -0.25, 0.0, 0.0, 0.0, 0.0), config);
    let high = heightfield_column_from_sample(&sample(0.0, 0.0, 0.75, 0.0, 0.0, 0.0, 0.0), config);

    assert_eq!(low.raw_surface_height_blocks, -512.0);
    assert_eq!(high.raw_surface_height_blocks, 1536.0);
}

#[test]
fn experimental_effective_range_saturates_outside_limits() {
    let config = HeightfieldConfig::default();
    let low = heightfield_column_from_sample(&sample(0.0, 0.0, -0.75, 0.0, 0.0, 0.0, 0.0), config);
    let high = heightfield_column_from_sample(&sample(0.0, 0.0, 1.25, 0.0, 0.0, 0.0, 0.0), config);

    assert_eq!(
        low.raw_surface_height_blocks,
        DEFAULT_HEIGHTFIELD_MIN_BLOCKS
    );
    assert_eq!(
        high.raw_surface_height_blocks,
        DEFAULT_HEIGHTFIELD_MAX_BLOCKS
    );
}

#[test]
fn signed_macro_zero_maps_to_sea_level_before_contour_snap() {
    let column = heightfield_column_from_sample(
        &sample(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        HeightfieldConfig::default(),
    );

    assert_eq!(
        column.raw_surface_height_blocks,
        DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS
    );
    assert_eq!(
        column.surface_height_blocks,
        DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS
    );
}

#[test]
fn small_positive_coastal_macro_height_starts_near_sea_level() {
    let column = heightfield_column_from_sample(
        &sample(0.0, 0.0, 0.0008, 0.0, 0.0, 0.0, 0.0),
        HeightfieldConfig::default(),
    );

    assert!(
        column.raw_surface_height_blocks <= 2.0,
        "signed macro height just above sea level should not become a high terrace: {}",
        column.raw_surface_height_blocks
    );
    assert!(
        column.surface_height_blocks <= 2.0,
        "signed macro height just above sea level should snap to the first few contour steps: {}",
        column.surface_height_blocks
    );
}

#[test]
fn changing_contour_step_snaps_land_surface_to_step_multiples() {
    let sample = sample(0.0, 0.0, 0.123, 0.0, 0.0, 0.0, 0.0);
    let column = heightfield_column_from_sample(
        &sample,
        HeightfieldConfig {
            contour: HeightfieldContourConfig {
                step_blocks: 4.0,
                min_gap_blocks: 0.0,
                river_min_gap_blocks: 0.0,
                band_smoothing: 0.0,
            },
            ..HeightfieldConfig::default()
        },
    );

    assert_eq!(column.contour_guided_surface_height_blocks % 4.0, 0.0);
    assert_eq!(column.surface_height_blocks % 4.0, 0.0);
}

#[test]
fn raw_continuous_height_is_recorded_but_final_uses_band_value() {
    let sample = sample(0.0, 0.0, 0.123, 0.0, 0.0, 0.0, 0.0);
    let column = heightfield_column_from_sample(&sample, HeightfieldConfig::default());

    assert_ne!(
        column.raw_surface_height_blocks,
        column.contour_guided_surface_height_blocks
    );
    assert_eq!(
        column.surface_height_blocks,
        column.contour_guided_surface_height_blocks
    );
}

#[test]
fn coast_mask_alone_does_not_smooth_contour_terrace() {
    let mut sample = sample(0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0);
    sample.coast_mask = 1.0;
    let column = heightfield_column_from_sample(&sample, HeightfieldConfig::default());
    let expected = resolve_contour_band_height(
        column.raw_surface_height_blocks,
        HeightfieldConfig::default().contour,
    );

    assert_eq!(column.surface_height_blocks, expected);
    assert_eq!(
        column.terrain_kind,
        HeightfieldTerrainKind::Land,
        "coast_mask is preserved as data but should not create a heightfield-specific terrain kind"
    );
    assert_eq!(
        column.water_y, None,
        "coast_mask alone should not create a water column"
    );
}

#[test]
fn coast_mask_negative_non_ocean_land_preserves_below_sea_bed_without_water() {
    let mut coast = sample(0.0, 0.0, -0.25, 0.0, 0.0, 0.0, 0.0);
    coast.coast_mask = 0.35;
    let column = heightfield_column_from_sample(&coast, HeightfieldConfig::default());

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Land);
    assert!(
        column.surface_y < 0,
        "negative coast-mask land should preserve its below-sea terrain bed: {}",
        column.surface_y
    );
    assert_eq!(column.water_y, None);
    assert_eq!(
        column.visible_surface_height_blocks(),
        column.surface_height_blocks
    );
}

#[test]
fn coastland_below_sea_gets_sea_level_water_without_ocean_kind() {
    let mut coast = sample(0.0, 0.0, -0.01, 0.0, 0.0, 0.0, 0.0);
    coast.surface_kind = Some(MacroSurfaceKind::CoastLand);
    coast.coast_mask = 1.0;
    let column = heightfield_column_from_sample(&coast, HeightfieldConfig::default());

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Coast);
    assert!(
        column.surface_y < 0,
        "below-sea coast land should preserve its terrain bed: {}",
        column.surface_y
    );
    assert_eq!(
        column.water_level_blocks,
        Some(DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS)
    );
    assert_eq!(column.water_y, Some(0));
}

#[test]
fn coastisland_below_sea_gets_sea_level_water_without_ocean_kind() {
    let mut coast = sample(0.0, 0.0, -0.01, 0.0, 0.0, 0.0, 0.0);
    coast.surface_kind = Some(MacroSurfaceKind::CoastIsland);
    coast.coast_mask = 1.0;
    let column = heightfield_column_from_sample(&coast, HeightfieldConfig::default());

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Coast);
    assert_eq!(column.water_y, Some(0));
}

#[test]
fn coastland_above_sea_gets_no_water() {
    let mut coast = sample(0.0, 0.0, 0.01, 0.0, 0.0, 0.0, 0.0);
    coast.surface_kind = Some(MacroSurfaceKind::CoastLand);
    coast.coast_mask = 1.0;
    let column = heightfield_column_from_sample(&coast, HeightfieldConfig::default());

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Coast);
    assert!(
        column.surface_y >= 0,
        "above-sea coast land should preserve its terrain bed: {}",
        column.surface_y
    );
    assert_eq!(column.water_y, None);
}

#[test]
fn non_coast_near_zero_land_keeps_ordinary_contour_behavior() {
    let land = sample(0.0, 0.0, 0.0004, 0.0, 0.0, 0.0, 0.0);
    let column = heightfield_column_from_sample(&land, HeightfieldConfig::default());

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Land);
    assert_eq!(column.surface_y, 0);
    assert_eq!(column.water_y, None);
}

#[test]
fn coast_near_zero_land_uses_ordinary_contour_behavior() {
    let mut coast = sample(0.0, 0.0, 0.0004, 0.0, 0.0, 0.0, 0.0);
    coast.coast_mask = 1.0;
    let column = heightfield_column_from_sample(&coast, HeightfieldConfig::default());

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Land);
    assert_eq!(column.surface_y, 0);
    assert_eq!(column.water_y, None);
    assert_eq!(column.coast_mask, 1.0);
}

#[test]
fn inland_negative_non_water_land_preserves_below_sea_bed() {
    let inland = sample(0.0, 0.0, -0.25, 0.0, 0.0, 0.0, 0.0);
    let column = heightfield_column_from_sample(&inland, HeightfieldConfig::default());

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Land);
    assert!(
        column.surface_y < 0,
        "negative non-water land should preserve its below-sea terrain bed: {}",
        column.surface_y
    );
    assert_eq!(column.water_y, None);
}

#[test]
fn water_columns_appear_for_submerged_ocean_and_lake_masks() {
    let tile = generate_heightfield_tile(&test_macro_tile(), HeightfieldConfig::default());

    assert!(tile.stats.ocean_column_count > 0);
    assert!(tile.stats.lake_column_count > 0);
    assert!(tile.stats.water_column_count > 0);
    assert!(
        tile.columns
            .iter()
            .filter(
                |column| matches!(column.terrain_kind, HeightfieldTerrainKind::Lake)
                    || (matches!(column.terrain_kind, HeightfieldTerrainKind::Ocean)
                        && column.surface_height_blocks < DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS)
            )
            .all(|column| column.water_level_blocks.is_some())
    );
    assert!(
        tile.columns
            .iter()
            .filter(
                |column| matches!(column.terrain_kind, HeightfieldTerrainKind::Ocean)
                    && column.surface_height_blocks >= DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS
            )
            .all(|column| column.water_level_blocks.is_none())
    );
}

#[test]
fn ocean_owned_positive_terrain_bed_has_no_water_column() {
    let column = heightfield_column_from_sample(
        &sample(0.0, 0.0, 0.01, 1.0, 0.0, 0.0, 0.0),
        HeightfieldConfig::default(),
    );

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
    assert!(
        column.surface_height_blocks >= DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
        "positive ocean-owned terrain bed should stay above sea level: {}",
        column.surface_height_blocks
    );
    assert_eq!(
        column.water_level_blocks, None,
        "water should only cover ocean-owned terrain when the final bed is below sea level"
    );
    assert_eq!(column.water_y, None);
}

#[test]
fn ocean_owned_negative_terrain_bed_has_sea_level_water() {
    let column = heightfield_column_from_sample(
        &sample(0.0, 0.0, -0.01, 1.0, 0.0, 0.0, 0.0),
        HeightfieldConfig::default(),
    );

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
    assert!(
        column.surface_height_blocks < DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
        "negative ocean-owned terrain bed should stay below sea level: {}",
        column.surface_height_blocks
    );
    assert_eq!(
        column.water_level_blocks,
        Some(DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS)
    );
    assert_eq!(column.water_y, Some(0));
}

#[test]
fn ocean_owned_positive_bed_with_perlin_has_no_water_column() {
    let sample = sample(53.0, -79.0, 0.01, 1.0, 0.0, 0.0, 0.0);
    let config = HeightfieldConfig {
        perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
        ..HeightfieldConfig::default()
    };
    let column = heightfield_column_from_sample(&sample, config);
    let mut land_sample = sample;
    land_sample.ocean_mask = 0.0;
    let land_column = heightfield_column_from_sample(&land_sample, config);

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
    assert_eq!(
        column.micro_relief_blocks, land_column.micro_relief_blocks,
        "above-sea ocean-owned dry terrain should use the same micro relief map as land"
    );
    assert_ne!(column.micro_relief_blocks, 0.0);
    assert_eq!(column.water_level_blocks, None);
    assert_eq!(column.water_y, None);
}

#[test]
fn ocean_owned_positive_bed_uses_land_perlin_not_ocean_bed_perlin() {
    let sample = sample(53.0, -79.0, 0.01, 1.0, 0.0, 0.0, 0.0);
    let enabled_config = HeightfieldConfig {
        perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
        ..HeightfieldConfig::default()
    };
    let disabled = heightfield_column_from_sample(&sample, HeightfieldConfig::default());
    let enabled = heightfield_column_from_sample(&sample, enabled_config);
    let mut land_sample = sample;
    land_sample.ocean_mask = 0.0;
    let land_enabled = heightfield_column_from_sample(&land_sample, enabled_config);

    assert_eq!(
        perlin::ocean_bed_relief_blocks(&sample, enabled_config.perlin),
        0.0,
        "ocean bed Perlin should be bed-only and stay inactive for above-sea ocean beds"
    );
    assert_eq!(disabled.water_level_blocks, None);
    assert_eq!(enabled.water_level_blocks, None);
    assert_eq!(enabled.water_y, None);
    assert_eq!(
        enabled.micro_relief_blocks, land_enabled.micro_relief_blocks,
        "above-sea ocean-owned dry terrain should reuse land micro relief"
    );
    assert_ne!(
        enabled.micro_relief_blocks, 0.0,
        "land-style Perlin should be present even if integer contour snapping keeps this column in the same band"
    );
}

#[test]
fn water_columns_do_not_appear_for_ocean_owned_above_sea_beds() {
    let config = MacroFieldTileConfig::new(0.0, 0.0, 2, 1, 32.0);
    let macro_tile = MacroFieldTile {
        config,
        samples: vec![
            sample(0.0, 0.0, 0.01, 1.0, 0.0, 0.0, 0.0),
            sample(32.0, 0.0, -0.01, 1.0, 0.0, 0.0, 0.0),
        ],
        stats: MacroFieldTileStats::default(),
    };

    let tile = generate_heightfield_tile(&macro_tile, HeightfieldConfig::default());

    assert_eq!(tile.column(0, 0).expect("above sea ocean").water_y, None);
    assert_eq!(tile.column(1, 0).expect("below sea ocean").water_y, Some(0));
    assert_eq!(tile.stats.water_column_count, 1);
}

#[test]
fn ocean_water_surface_is_sea_level_while_bed_preserves_source_height() {
    let tile = generate_heightfield_tile(&test_macro_tile(), HeightfieldConfig::default());

    assert!(tile.stats.ocean_column_count > 0);
    assert_eq!(
        tile.stats.min_ocean_visible_surface_blocks,
        DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS
    );
    assert_eq!(
        tile.stats.max_ocean_visible_surface_blocks,
        DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS
    );
    assert!(
        tile.columns
            .iter()
            .filter(|column| matches!(column.terrain_kind, HeightfieldTerrainKind::Ocean))
            .all(|column| {
                column.surface_height_blocks < DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS
                    && column.water_level_blocks == Some(DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS)
                    && column.visible_surface_height_blocks()
                        == DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS
            })
    );
}

#[test]
fn ocean_negative_source_bed_stays_below_sea_level_with_water_to_zero() {
    let ocean = sample(0.0, 0.0, -0.25, 1.0, 0.0, 0.0, 0.0);
    let column = heightfield_column_from_sample(&ocean, HeightfieldConfig::default());

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
    assert!(
        column.surface_y < 0,
        "ocean bed should preserve negative source terrain instead of clamping to sea level: {}",
        column.surface_y
    );
    assert_eq!(column.water_y, Some(0));
    assert_eq!(
        column.visible_surface_height_blocks(),
        DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS
    );
}

#[test]
fn ocean_nonnegative_source_bed_is_preserved_without_fallback() {
    let column = heightfield_column_from_sample(
        &sample(0.0, 0.0, 0.01, 1.0, 0.0, 0.0, 0.0),
        HeightfieldConfig::default(),
    );

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
    assert_eq!(
        column.water_level_blocks, None,
        "ocean-owned terrain above sea level should not create a water column"
    );
    assert!(
        column.surface_height_blocks >= DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
        "heightfield should preserve source ocean bed directly instead of inventing a below-sea fallback: {}",
        column.surface_height_blocks
    );
}

#[test]
fn lake_water_surface_uses_lake_bed_depth_instead_of_absolute_sea_level() {
    let mut lake = sample(0.0, 0.0, 0.05, 0.0, 1.0, 0.0, 0.0);
    lake.combined_macro_height = 0.04;
    let column = heightfield_column_from_sample(&lake, HeightfieldConfig::default());

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Lake);
    assert!(
        column.water_level_blocks.expect("lake water") > DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
        "inland lake water should derive from the lake source elevation, not y=0"
    );
    assert!(
        column.surface_height_blocks < column.water_level_blocks.expect("lake water"),
        "lake bed should sit below the water surface"
    );
    assert!(
        column.water_level_blocks.expect("lake water") < column.raw_surface_height_blocks + 8.0,
        "lake water should stay tied to the carved bed instead of riding high on the source terrain"
    );
    assert_eq!(
        column.visible_surface_height_blocks(),
        column.water_level_blocks.expect("lake water")
    );
}

#[test]
fn lake_bed_preserves_local_relief_instead_of_flattening_to_one_plane() {
    let mut shallow = sample(0.0, 0.0, 0.05, 0.0, 1.0, 0.0, 0.0);
    shallow.combined_macro_height = 0.047;
    let mut deep = sample(1.0, 0.0, 0.05, 0.0, 1.0, 0.0, 0.0);
    deep.combined_macro_height = 0.038;

    let shallow = heightfield_column_from_sample(&shallow, HeightfieldConfig::default());
    let deep = heightfield_column_from_sample(&deep, HeightfieldConfig::default());

    assert_eq!(shallow.water_y, deep.water_y);
    assert!(
        deep.surface_height_blocks < shallow.surface_height_blocks,
        "lake bed should preserve U-shaped bed variation instead of a completely flat floor: shallow={} deep={}",
        shallow.surface_height_blocks,
        deep.surface_height_blocks
    );
    assert!(
        shallow.water_y.expect("lake water") - deep.surface_y <= 24,
        "lake bed carve should stay depth-capped instead of making an abrupt shaft"
    );
}

#[test]
fn coast_adjacent_land_preserves_contour_band_without_extra_clamp() {
    let config = MacroFieldTileConfig::new(0.0, 0.0, 3, 1, 32.0);
    let samples = vec![
        sample(0.0, 0.0, -0.8, 1.0, 0.0, 0.0, 0.0),
        sample(32.0, 0.0, 0.00049, 0.0, 0.0, 0.0, 0.0),
        sample(64.0, 0.0, 0.00098, 0.0, 0.0, 0.0, 0.0),
    ];
    let macro_tile = MacroFieldTile {
        config,
        samples,
        stats: MacroFieldTileStats::default(),
    };
    let tile = generate_heightfield_tile(&macro_tile, HeightfieldConfig::default());
    let coast = tile.column(1, 0).expect("coast column");
    let inland = tile.column(2, 0).expect("inland column");

    assert_eq!(coast.surface_height_blocks, 1.0);
    assert_eq!(inland.surface_height_blocks, 2.0);
    assert!(coast.surface_height_blocks > DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS);
    assert!(inland.surface_height_blocks > coast.surface_height_blocks);
    assert_eq!(
        tile.stats.max_shore_visible_neighbor_delta_blocks, DEFAULT_HEIGHTFIELD_CONTOUR_STEP_BLOCKS,
        "standing-water shoreline should only reflect the ordinary contour step"
    );
}

#[test]
fn water_visible_top_uses_water_surface_not_bed() {
    let sample = sample(0.0, 0.0, -0.8, 1.0, 0.0, 0.0, 0.0);
    let column = heightfield_column_from_sample(&sample, HeightfieldConfig::default());

    assert!(
        column.surface_height_blocks < DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
        "standing ocean bed should stay below the visible water surface: {}",
        column.surface_height_blocks
    );
    assert_eq!(
        column.water_level_blocks,
        Some(DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS)
    );
    assert_eq!(
        column.visible_surface_height_blocks(),
        DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS
    );
}

#[test]
fn final_visible_heights_are_integer_blocks() {
    let tile = generate_heightfield_tile(&test_macro_tile(), HeightfieldConfig::default());

    assert!(tile.columns.iter().all(|column| {
        column.visible_surface_height_blocks().fract() == 0.0
            && column
                .water_level_blocks
                .is_none_or(|water| water.fract() == 0.0)
            && column
                .river_water_height_blocks
                .is_none_or(|water| water.fract() == 0.0)
    }));
}

#[test]
fn river_water_steps_down_to_standing_water_without_large_jumps() {
    let config = MacroFieldTileConfig::new(0.0, 0.0, 5, 1, 32.0);
    let mut first = sample_with_river(0.0, 0.0, 0.006, 0.22);
    first.river_bed_depth_hint = 0.25;
    let mut second = sample_with_river(32.0, 0.0, 0.005, 0.42);
    second.river_bed_depth_hint = 0.35;
    let mut third = sample_with_river(64.0, 0.0, 0.004, 0.62);
    third.river_bed_depth_hint = 0.45;
    let mut fourth = sample_with_river(96.0, 0.0, 0.003, 0.82);
    fourth.river_bed_depth_hint = 0.55;
    let samples = vec![
        first,
        second,
        third,
        fourth,
        sample(128.0, 0.0, -0.8, 1.0, 0.0, 0.0, 0.0),
    ];
    let macro_tile = MacroFieldTile {
        config,
        samples,
        stats: MacroFieldTileStats::default(),
    };
    let tile = generate_heightfield_tile(&macro_tile, HeightfieldConfig::default());
    let river_water = (0..4)
        .map(|x| {
            tile.column(x, 0)
                .expect("river column")
                .river_water_height_blocks
                .expect("river water")
        })
        .collect::<Vec<_>>();

    assert_eq!(
        tile.column(4, 0).expect("ocean").water_level_blocks,
        Some(DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS)
    );
    assert!(
        river_water.windows(2).all(|pair| pair[0] >= pair[1]),
        "river water should not climb as display flow increases: {river_water:?}"
    );
    assert!(
        river_water
            .windows(2)
            .all(|pair| (pair[0] - pair[1]).abs() <= 1.0),
        "river water should descend by at most one block per neighboring sample: {river_water:?}"
    );
    assert!(
        tile.stats.max_river_water_neighbor_delta_blocks <= 1.0,
        "river water neighbor delta should be constrained: {}",
        tile.stats.max_river_water_neighbor_delta_blocks
    );
    assert_eq!(tile.stats.river_uphill_flow_neighbor_count, 0);
}

#[test]
fn river_water_descent_suppresses_water_without_recutting_resolved_bed() {
    let config = MacroFieldTileConfig::new(0.0, 0.0, 2, 1, 32.0);
    let mut river = sample_with_river(0.0, 0.0, 0.010, 0.22);
    river.river_bed_depth_hint = 0.03;
    let resolved_river = heightfield_column_from_sample(&river, HeightfieldConfig::default());
    let samples = vec![river, sample(32.0, 0.0, -0.8, 1.0, 0.0, 0.0, 0.0)];
    let macro_tile = MacroFieldTile {
        config,
        samples,
        stats: MacroFieldTileStats::default(),
    };

    let tile = generate_heightfield_tile(&macro_tile, HeightfieldConfig::default());
    let river = tile.column(0, 0).expect("river column");

    assert_eq!(river.terrain_kind, HeightfieldTerrainKind::River);
    assert_eq!(
        river.surface_y, resolved_river.surface_y,
        "river water descent must not recut terrain bed after column resolve"
    );
    assert_eq!(
        river.water_y, None,
        "water that descends to the resolved bed or below should be suppressed instead"
    );
    assert_eq!(river.water_level_blocks, None);
    assert_eq!(river.river_water_height_blocks, None);
}

#[test]
fn headwater_river_bed_stays_shallow_and_nearly_filled() {
    let mut headwater = sample_with_river(0.0, 0.0, 0.004, 0.05);
    headwater.river_bed_depth_hint = 3.0 / 40.0;
    let column = heightfield_column_from_sample(&headwater, HeightfieldConfig::default());

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::River);
    assert!(
        column.river_bed_depth_blocks <= 3.0,
        "headwater stream carve should stay within the small V-cut range: {}",
        column.river_bed_depth_blocks
    );
    assert!(
        column.water_y.expect("river water") - column.surface_y <= 2,
        "shallow headwater water should sit close to the surrounding bed instead of exposing a deep trench: bed={} water={:?}",
        column.surface_y,
        column.water_y
    );
}

#[test]
fn river_bed_depth_scales_with_flow_hint() {
    let mut headwater = sample_with_river(0.0, 0.0, 0.02, 0.05);
    headwater.river_bed_depth_hint = 3.0 / 40.0;
    let mut lower = sample_with_river(1.0, 0.0, 0.02, 0.85);
    lower.river_bed_depth_hint = 18.0 / 40.0;

    let headwater = heightfield_column_from_sample(&headwater, HeightfieldConfig::default());
    let lower = heightfield_column_from_sample(&lower, HeightfieldConfig::default());

    assert!(
        lower.river_bed_depth_blocks > headwater.river_bed_depth_blocks * 3.0,
        "downstream river bed depth should follow river-plan Q scale: headwater={} lower={}",
        headwater.river_bed_depth_blocks,
        lower.river_bed_depth_blocks
    );
    assert!(
        lower.water_y.expect("lower water") - lower.surface_y
            > headwater.water_y.expect("headwater water") - headwater.surface_y,
        "larger Q should allow deeper water while keeping the surface separate from bed"
    );
}

#[test]
fn deterministic_river_bed_variation_does_not_move_water_surface() {
    let mut left = sample_with_river(37.0, -91.0, 0.20, 0.62);
    left.river_bed_depth_hint = 0.46;
    left.river_bank_roughness_hint = 0.80;
    left.river_gravel_hint = 0.55;
    let mut right = left;
    right.position = WorldPlanePoint::new(53.0, -91.0);

    let left_variation = deterministic_river_bed_variation_blocks(&left);
    let right_variation = deterministic_river_bed_variation_blocks(&right);
    let left_column = heightfield_column_from_sample(&left, HeightfieldConfig::default());
    let right_column = heightfield_column_from_sample(&right, HeightfieldConfig::default());

    assert_ne!(
        left_variation, right_variation,
        "river bed variation should be deterministic but not uniform across neighboring bed samples"
    );
    assert_eq!(
        left_column.water_y, right_column.water_y,
        "bed variation must not perturb the river water surface"
    );
}

#[test]
fn river_bed_variation_survives_integer_snap_across_bed_samples() {
    let config = HeightfieldConfig::default();
    let mut base = sample_with_river(0.0, 0.0, 0.20, 0.72);
    base.river_bed_depth_hint = 0.42;
    base.river_bank_roughness_hint = 0.90;
    base.river_gravel_hint = 0.70;
    base.river_cutbank_hint = 0.55;

    let surfaces = [0.0, 11.0, 23.0, 37.0, 52.0, 71.0]
        .into_iter()
        .map(|x| {
            let mut sample = base;
            sample.position = WorldPlanePoint::new(x, -91.0);
            heightfield_column_from_sample(&sample, config).surface_y
        })
        .collect::<Vec<_>>();
    let first = surfaces[0];

    assert!(
        surfaces.iter().any(|surface| *surface != first),
        "river bed variation should be strong enough to survive 1-block snapping: {surfaces:?}"
    );
}

#[test]
fn deterministic_river_bank_variation_is_available_by_default() {
    let mut bank = sample_with_river(41.0, 19.0, 0.24, 0.42);
    bank.river_core_strength = 0.0;
    bank.river_shoulder_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD * 0.70;
    bank.river_valley_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD * 0.70;
    bank.river_distance_blocks = 32.0;
    bank.river_bank_roughness_hint = 0.90;
    bank.river_gravel_hint = 0.60;

    assert_ne!(
        deterministic_river_bank_variation_blocks(&bank),
        0.0,
        "river shoulders should have deterministic default relief even when optional Perlin is disabled"
    );
}

#[test]
fn river_shoulder_strength_does_not_create_water_or_bed_core() {
    let mut shoulder = sample(64.0, 0.0, 0.18, 0.0, 0.0, 0.0, 0.0);
    shoulder.river_core_strength = 0.0;
    shoulder.river_shoulder_strength = 0.76;
    shoulder.river_valley_strength = 0.76;
    shoulder.river_distance_blocks = 72.0;
    shoulder.river_flow_hint = 0.72;
    shoulder.river_bed_depth_hint = 0.9;
    shoulder.river_bank_roughness_hint = 0.7;

    let column = heightfield_column_from_sample(&shoulder, HeightfieldConfig::default());

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Land);
    assert_eq!(column.water_y, None);
    assert_eq!(column.river_bed_depth_blocks, 0.0);
}

#[test]
fn high_shoulder_lower_river_bank_slopes_toward_core_without_water() {
    let mut bank = sample_with_river(0.0, 0.0, 47.0 / DEFAULT_HEIGHTFIELD_MAX_BLOCKS, 0.73);
    bank.river_core_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD * 0.40;
    bank.river_shoulder_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD;
    bank.river_valley_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD;
    bank.river_distance_blocks = 38.0;
    bank.river_bed_depth_hint = 0.19;
    bank.river_bank_roughness_hint = 0.5;

    let mut plain = bank;
    plain.river_core_strength = 0.0;
    plain.river_shoulder_strength = 0.0;
    plain.river_valley_strength = 0.0;
    plain.river_flow_hint = 0.0;
    plain.river_distance_blocks = f32::INFINITY;
    plain.river_bed_depth_hint = 0.0;
    plain.river_bank_roughness_hint = 0.0;

    let bank_column = heightfield_column_from_sample(&bank, HeightfieldConfig::default());
    let plain_column = heightfield_column_from_sample(&plain, HeightfieldConfig::default());

    assert_eq!(bank_column.terrain_kind, HeightfieldTerrainKind::Land);
    assert_eq!(bank_column.water_level_blocks, None);
    assert_eq!(bank_column.river_bed_depth_blocks, 0.0);
    assert!(
        plain_column.surface_y - bank_column.surface_y >= 3,
        "high river shoulder just outside the core should form a sloped lower bank instead of an uncut wall: plain={} bank={}",
        plain_column.surface_y,
        bank_column.surface_y
    );
}

#[test]
fn broad_river_surroundings_do_not_get_default_bank_noise() {
    let mut broad = sample_with_river(160.0, 24.0, 0.24, 0.62);
    broad.river_core_strength = 0.0;
    broad.river_shoulder_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD * 0.22;
    broad.river_valley_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD * 0.22;
    broad.river_distance_blocks = 176.0;
    broad.river_bank_roughness_hint = 1.0;
    broad.river_gravel_hint = 1.0;
    let mut plain = broad;
    plain.river_core_strength = 0.0;
    plain.river_shoulder_strength = 0.0;
    plain.river_valley_strength = 0.0;
    plain.river_distance_blocks = f32::INFINITY;
    plain.river_flow_hint = 0.0;

    let broad_column = heightfield_column_from_sample(&broad, HeightfieldConfig::default());
    let plain_column = heightfield_column_from_sample(&plain, HeightfieldConfig::default());

    assert_eq!(
        deterministic_river_bank_variation_blocks(&broad),
        0.0,
        "broad surrounding valley should not receive deterministic bank relief"
    );
    assert_eq!(
        broad_column.surface_y, plain_column.surface_y,
        "broad surrounding terrain should keep the same contour surface as non-river land"
    );
}

#[test]
fn preview_perlin_does_not_reintroduce_broad_river_bank_noise() {
    let config = HeightfieldConfig {
        perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
        ..HeightfieldConfig::default()
    };
    let mut broad = sample_with_river(192.0, -48.0, 0.31, 0.74);
    broad.river_core_strength = 0.0;
    broad.river_shoulder_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD * 0.25;
    broad.river_valley_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD * 0.25;
    broad.river_distance_blocks = 160.0;
    broad.river_bank_roughness_hint = 1.0;
    broad.river_gravel_hint = 1.0;

    assert_eq!(
        perlin::river_bank_relief_blocks(&broad, config.perlin),
        0.0,
        "optional Perlin bank relief must stay local to the selected river bank"
    );
}

#[test]
fn river_bed_hint_can_cut_below_sea_level_at_ocean_mouth() {
    let config = MacroFieldTileConfig::new(0.0, 0.0, 2, 1, 1.0);
    let mut river = sample_with_river(0.0, 0.0, 0.0, 0.95);
    river.river_bed_depth_hint = 0.85;
    river.river_bank_roughness_hint = 0.2;
    let samples = vec![river, sample(1.0, 0.0, -0.8, 1.0, 0.0, 0.0, 0.0)];
    let macro_tile = MacroFieldTile {
        config,
        samples,
        stats: MacroFieldTileStats::default(),
    };

    let tile = generate_heightfield_tile(&macro_tile, HeightfieldConfig::default());
    let river = tile.column(0, 0).expect("river mouth");
    let ocean = tile.column(1, 0).expect("ocean");

    assert_eq!(ocean.visible_surface_height_blocks(), 0.0);
    assert_eq!(river.terrain_kind, HeightfieldTerrainKind::River);
    assert!(
        river.surface_height_blocks < DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
        "river bed should stay carved below sea level near the mouth: {}",
        river.surface_height_blocks
    );
    assert!(
        river.water_level_blocks.unwrap_or_default() >= DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
        "river water surface should remain distinct from the carved bed"
    );
    assert!(
        river.river_bed_depth_blocks > 0.0,
        "heightfield should preserve the river bed depth diagnostic"
    );
}

#[test]
fn selected_river_bed_hint_can_cut_ocean_column_below_sea_level() {
    let mut mouth = sample_with_river(0.0, 0.0, -0.25, 0.98);
    mouth.ocean_mask = 1.0;
    mouth.river_bed_depth_hint = 0.9;

    let column = heightfield_column_from_sample(&mouth, HeightfieldConfig::default());

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
    assert_eq!(
        column.water_level_blocks,
        Some(DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS),
        "ocean mouth water surface should stay at sea level"
    );
    assert!(
        column.surface_height_blocks < DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
        "selected river bed hint should carve the ocean-mouth bed below y=0: {}",
        column.surface_height_blocks
    );
    assert_eq!(
        column.visible_surface_height_blocks(),
        DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
        "visible standing-water top remains the sea surface, not the carved bed"
    );
}

#[test]
fn ocean_owned_above_sea_river_hint_keeps_ocean_ownership_without_mouth_carve() {
    let mut mouth = sample_with_river(0.0, 0.0, 0.004, 0.96);
    mouth.ocean_mask = 1.0;
    mouth.river_core_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD * 0.65;
    mouth.river_shoulder_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD * 0.65;
    mouth.river_valley_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD * 0.65;
    mouth.river_bed_depth_hint = 0.82;
    mouth.river_bank_roughness_hint = 0.2;

    let column = heightfield_column_from_sample(&mouth, HeightfieldConfig::default());

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
    assert_eq!(
        column.water_level_blocks, None,
        "above-sea ocean-owned source keeps ocean ownership without adding river water"
    );
    assert!(
        column.surface_height_blocks >= DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
        "river hint must not carve an above-sea ocean-owned column below y=0: {}",
        column.surface_height_blocks
    );
    assert_eq!(
        column.river_bed_depth_blocks, 0.0,
        "ocean-owned columns are filled as ocean before river policy runs"
    );
}

#[test]
fn high_core_above_sea_ocean_column_does_not_threshold_cut_mouth_bed() {
    let mut mouth = sample_with_river(0.0, 0.0, 0.005, 0.96);
    mouth.ocean_mask = 1.0;
    mouth.river_core_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD + 0.01;
    mouth.river_shoulder_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD + 0.01;
    mouth.river_valley_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD + 0.01;
    mouth.river_distance_blocks = 38.0;
    mouth.river_bed_depth_hint = 0.82;
    mouth.river_bank_roughness_hint = 0.62;

    let column = heightfield_column_from_sample(&mouth, HeightfieldConfig::default());

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
    assert_eq!(
        column.water_level_blocks, None,
        "above-sea ocean source should not create standing water only because river core crosses the threshold"
    );
    assert!(
        column.surface_height_blocks >= DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
        "above-sea ocean source should preserve its source bed instead of threshold-cutting to a river trench: {}",
        column.surface_height_blocks
    );
    assert_eq!(
        column.river_bed_depth_blocks, 0.0,
        "river bed depth is reserved for non-ocean river columns or below-sea standing-water mouth beds"
    );
}

#[test]
fn low_strength_river_hint_does_not_override_above_sea_ocean_column() {
    let mut mouth = sample_with_river(0.0, 0.0, 0.004, 0.96);
    mouth.ocean_mask = 1.0;
    mouth.river_core_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD * 0.35;
    mouth.river_shoulder_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD * 0.35;
    mouth.river_valley_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD * 0.35;
    mouth.river_distance_blocks = 96.0;
    mouth.river_bed_depth_hint = 0.82;

    let column = heightfield_column_from_sample(&mouth, HeightfieldConfig::default());

    assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
    assert_eq!(
        column.water_level_blocks, None,
        "low-strength river hint must not create river water over an above-sea ocean column"
    );
    assert!(
        column.surface_height_blocks >= DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
        "low-strength mouth hint must not cut above-sea ocean source below sea level: {}",
        column.surface_height_blocks
    );
    assert_eq!(column.river_bed_depth_blocks, 0.0);
}

#[test]
fn water_adjacent_visible_step_stays_under_two_blocks() {
    let config = MacroFieldTileConfig::new(0.0, 0.0, 3, 1, 1.0);
    let samples = vec![
        sample(0.0, 0.0, -0.004, 1.0, 0.0, 0.0, 0.0),
        sample(1.0, 0.0, 0.0005, 0.0, 0.0, 0.0, 0.0),
        sample(2.0, 0.0, 0.0009, 0.0, 0.0, 0.0, 0.0),
    ];
    let macro_tile = MacroFieldTile {
        config,
        samples,
        stats: MacroFieldTileStats::default(),
    };

    let tile = generate_heightfield_tile(&macro_tile, HeightfieldConfig::default());

    assert!(
        max_visible_step_adjacent_to_water(&tile) < 2.0,
        "water-adjacent visible terrain should not form a vertical step of 2+ blocks"
    );
}

#[test]
fn same_context_downstream_river_bed_step_is_limited() {
    let config = MacroFieldTileConfig::new(-1875.5, -779.5, 2, 1, 1.0);
    let mut left = sample_with_river(-1875.5, -779.5, 0.021431, 0.663);
    left.river_core_strength = 0.890;
    left.river_shoulder_strength = 0.890;
    left.river_valley_strength = 0.890;
    left.river_distance_blocks = 35.751;
    left.river_bed_depth_hint = 0.171;
    left.river_bank_roughness_hint = 0.625;
    left.river_gravel_hint = 0.431;
    let mut right = sample_with_river(-1874.5, -779.5, 0.022834, 0.663);
    right.river_core_strength = 0.890;
    right.river_shoulder_strength = 0.890;
    right.river_valley_strength = 0.890;
    right.river_distance_blocks = 35.536;
    right.river_bed_depth_hint = 0.171;
    right.river_bank_roughness_hint = 0.625;
    right.river_gravel_hint = 0.431;
    let macro_tile = MacroFieldTile {
        config,
        samples: vec![left, right],
        stats: MacroFieldTileStats::default(),
    };

    let tile = generate_heightfield_tile(&macro_tile, HeightfieldConfig::default());
    let left = tile.column(0, 0).expect("left river column");
    let right = tile.column(1, 0).expect("right river column");
    let bed_delta = (left.surface_y - right.surface_y).abs();

    assert_eq!(left.terrain_kind, HeightfieldTerrainKind::River);
    assert_eq!(right.terrain_kind, HeightfieldTerrainKind::River);
    assert!(
        bed_delta <= 1,
        "same-context downstream river beds should not keep a 3-block contour-source cross-section step: left={} right={}",
        left.surface_y,
        right.surface_y
    );
    assert!(
        (left.visible_surface_height_blocks() - right.visible_surface_height_blocks()).abs() <= 1.0,
        "water top should remain locally continuous while the bed step is limited"
    );
}

#[test]
fn raw_and_contour_heights_are_recorded_before_snap() {
    let mut sample = sample(0.0, 0.0, 0.123, 0.0, 0.0, 0.0, 0.0);
    sample.coast_mask = 0.65;
    let column = heightfield_column_from_sample(&sample, HeightfieldConfig::default());

    assert_ne!(
        column.raw_surface_height_blocks,
        column.surface_height_blocks
    );
    assert_eq!(
        column.constrained_surface_height_blocks,
        column.contour_guided_surface_height_blocks
    );
    assert_eq!(column.surface_height_blocks, column.surface_y as f32);
}

fn test_macro_tile() -> MacroFieldTile {
    let config = MacroFieldTileConfig::new(0.0, 0.0, 3, 2, 32.0);
    let samples = vec![
        sample(0.0, 0.0, -0.8, 1.0, 0.0, 0.0, 0.0),
        sample(32.0, 0.0, -0.25, 0.0, 1.0, 0.0, 0.0),
        sample(64.0, 0.0, 0.05, 0.0, 0.0, 1.0, 0.0),
        sample(0.0, 32.0, 0.35, 0.0, 0.0, 0.0, 0.0),
        sample(32.0, 32.0, 0.7, 0.0, 0.0, 0.0, 0.8),
        sample(64.0, 32.0, 1.0, 0.0, 0.0, 0.0, 0.0),
    ];

    MacroFieldTile {
        config,
        samples,
        stats: MacroFieldTileStats::default(),
    }
}

fn max_visible_step_adjacent_to_water(tile: &HeightfieldTile) -> f32 {
    let width = tile.width as usize;
    let height = tile.height as usize;
    let mut max_delta = 0.0f32;
    for (index, column) in tile.columns.iter().enumerate() {
        for neighbor in neighbor_indices(index, width, height) {
            let neighbor_column = tile.columns[neighbor];
            if column.has_water_column() || neighbor_column.has_water_column() {
                max_delta = max_delta.max(
                    (column.visible_surface_height_blocks()
                        - neighbor_column.visible_surface_height_blocks())
                    .abs(),
                );
            }
        }
    }
    max_delta
}

fn sample(
    x: f32,
    z: f32,
    height: f32,
    ocean: f32,
    lake: f32,
    dry: f32,
    ridge: f32,
) -> MacroFieldSample {
    MacroFieldSample {
        position: WorldPlanePoint::new(x, z),
        nearest_site: None,
        surface_kind: None,
        biome_context: None,
        biome: None,
        macro_elevation: height,
        ocean_mask: ocean,
        coast_mask: 0.0,
        lake_mask: lake,
        dry_basin_mask: dry,
        ridge_influence: ridge,
        river_core_strength: 0.0,
        river_shoulder_strength: 0.0,
        river_valley_strength: 0.0,
        river_distance_blocks: f32::INFINITY,
        river_flow_hint: 0.0,
        river_longitudinal_blocks: 0.0,
        river_bed_depth_hint: 0.0,
        river_bank_roughness_hint: 0.0,
        river_gravel_hint: 0.0,
        river_cutbank_hint: 0.0,
        combined_macro_height: height,
    }
}

fn sample_with_river(x: f32, z: f32, height: f32, flow_hint: f32) -> MacroFieldSample {
    let mut sample = sample(x, z, height, 0.0, 0.0, 0.0, 0.0);
    sample.river_core_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD;
    sample.river_shoulder_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD;
    sample.river_valley_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD;
    sample.river_flow_hint = flow_hint;
    sample
}
