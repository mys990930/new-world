use super::types::MacroFieldTileConfig;
use crate::world::generation::graph::WorldPlanePoint;
use crate::world::generation::macro_map::{MacroSite, MacroSurfaceKind};

pub(super) const RIDGE_INFLUENCE_VISIBLE_FLOOR: f32 = 0.12;
const ESTUARY_FAN_FINAL_DEPTH_SCALE: f32 = 0.18;
const ESTUARY_FAN_ENTRY_DEPTH_SCALE: f32 = 0.28;
const ESTUARY_FAN_TERMINAL_FLOOR_GUARD_SCALE: f32 = 0.34;
const ESTUARY_FAN_DEPTH_GRADE_BLOCKS_PER_BLOCK: f32 = 0.36;
const RIVER_CORE_HEIGHT_PROFILE_THRESHOLD: f32 = 0.88;
pub(super) fn lake_boundary_lowering_factor(
    primary: MacroSite,
    distance_to_curve_blocks: f32,
    blend_radius_blocks: f32,
    roughness_offset_blocks: f32,
) -> f32 {
    let roughened_distance = (distance_to_curve_blocks + roughness_offset_blocks).max(0.0);
    let away_from_boundary =
        smoothstep01(roughened_distance / blend_radius_blocks.max(f32::EPSILON));
    if is_lake_surface(primary.surface_kind) {
        away_from_boundary
    } else {
        0.0
    }
}

#[cfg(test)]
pub(super) fn combine_macro_height(
    macro_elevation: f32,
    ocean_mask: f32,
    _coast_mask: f32,
    _lake_mask: f32,
    _dry_basin_mask: f32,
    lake_lowering_factor: f32,
    ridge_influence: f32,
    river_shoulder_strength: f32,
    river_flow_hint: f32,
    config: MacroFieldTileConfig,
) -> f32 {
    combine_macro_height_with_river_longitudinal(
        macro_elevation,
        ocean_mask,
        _coast_mask,
        _lake_mask,
        _dry_basin_mask,
        lake_lowering_factor,
        ridge_influence,
        river_shoulder_strength,
        river_flow_hint,
        None,
        f32::NAN,
        config,
    )
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub(super) fn combine_macro_height_with_river_longitudinal(
    macro_elevation: f32,
    ocean_mask: f32,
    _coast_mask: f32,
    _lake_mask: f32,
    _dry_basin_mask: f32,
    lake_lowering_factor: f32,
    ridge_influence: f32,
    river_shoulder_strength: f32,
    river_flow_hint: f32,
    river_centerline_macro_elevation: Option<f32>,
    river_longitudinal_blocks: f32,
    config: MacroFieldTileConfig,
) -> f32 {
    combine_macro_height_with_river_profile(
        macro_elevation,
        ocean_mask,
        _coast_mask,
        _lake_mask,
        _dry_basin_mask,
        lake_lowering_factor,
        ridge_influence,
        river_shoulder_strength,
        0.0,
        river_flow_hint,
        0.0,
        0.0,
        0.0,
        0.0,
        river_centerline_macro_elevation,
        river_longitudinal_blocks,
        None,
        config,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn combine_macro_height_with_river_profile(
    macro_elevation: f32,
    ocean_mask: f32,
    _coast_mask: f32,
    _lake_mask: f32,
    _dry_basin_mask: f32,
    lake_lowering_factor: f32,
    ridge_influence: f32,
    river_shoulder_strength: f32,
    river_core_strength: f32,
    river_flow_hint: f32,
    river_core_depth_hint: f32,
    river_bank_roughness_hint: f32,
    river_gravel_hint: f32,
    river_cutbank_hint: f32,
    river_centerline_macro_elevation: Option<f32>,
    river_longitudinal_blocks: f32,
    river_position: Option<WorldPlanePoint>,
    config: MacroFieldTileConfig,
) -> f32 {
    combine_macro_height_with_estuary_profile(
        macro_elevation,
        ocean_mask,
        _coast_mask,
        _lake_mask,
        _dry_basin_mask,
        lake_lowering_factor,
        ridge_influence,
        river_shoulder_strength,
        river_core_strength,
        river_flow_hint,
        river_core_depth_hint,
        river_bank_roughness_hint,
        river_gravel_hint,
        river_cutbank_hint,
        river_centerline_macro_elevation,
        river_longitudinal_blocks,
        river_position,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        config,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn combine_macro_height_with_estuary_profile(
    macro_elevation: f32,
    ocean_mask: f32,
    coast_mask: f32,
    _lake_mask: f32,
    _dry_basin_mask: f32,
    lake_lowering_factor: f32,
    ridge_influence: f32,
    river_shoulder_strength: f32,
    river_core_strength: f32,
    river_flow_hint: f32,
    river_core_depth_hint: f32,
    river_bank_roughness_hint: f32,
    river_gravel_hint: f32,
    river_cutbank_hint: f32,
    river_centerline_macro_elevation: Option<f32>,
    river_longitudinal_blocks: f32,
    river_position: Option<WorldPlanePoint>,
    estuary_strength: f32,
    estuary_water_strength: f32,
    estuary_flow_hint: f32,
    estuary_bed_depth_hint: f32,
    estuary_along_blocks: f32,
    config: MacroFieldTileConfig,
) -> f32 {
    let ridge_raise = ridge_influence * config.ridge_height_scale;
    let shoulder_height = river_shoulder_context_height_with_position(
        macro_elevation,
        river_shoulder_strength,
        river_flow_hint,
        river_centerline_macro_elevation,
        river_longitudinal_blocks,
        river_position,
        config,
    );
    let core_budget = river_core_downcut_budget(
        river_flow_hint,
        river_core_depth_hint,
        river_position,
        config,
    ) * river_morphology_lowering_scale(
        river_position,
        river_flow_hint,
        river_bank_roughness_hint,
        river_gravel_hint,
        river_cutbank_hint,
    );
    let bank_lowering = river_bed_bank_lowering(
        river_shoulder_strength,
        river_core_strength,
        river_flow_hint,
        core_budget,
    );
    let core_lowering =
        river_core_profile_lowering(river_core_strength, river_flow_hint, core_budget);
    let gravel_bar_fill = river_gravel_bar_fill(
        river_core_strength,
        river_flow_hint,
        river_gravel_hint,
        core_budget,
    );
    let core_lowering = (core_lowering - gravel_bar_fill).max(0.0);
    let river_context_height =
        (shoulder_height - bank_lowering.max(core_lowering)).min(shoulder_height);
    let mut height = river_context_height + ridge_raise;
    height = estuary_fan_macro_height(
        height,
        macro_elevation,
        ocean_mask,
        coast_mask,
        lake_lowering_factor,
        _dry_basin_mask,
        estuary_strength,
        estuary_water_strength,
        estuary_flow_hint,
        estuary_bed_depth_hint,
        estuary_along_blocks,
        config,
    );
    if ocean_mask > 0.5 {
        height = ocean_bathymetry_macro_height(height);
    } else if lake_lowering_factor > 0.0 {
        height = lake_bed_macro_height(height, lake_lowering_factor, config);
    }
    height.clamp(-2.0, 2.0)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn estuary_fan_macro_height(
    height: f32,
    macro_elevation: f32,
    ocean_mask: f32,
    coast_mask: f32,
    lake_lowering_factor: f32,
    dry_basin_mask: f32,
    estuary_strength: f32,
    estuary_water_strength: f32,
    estuary_flow_hint: f32,
    estuary_bed_depth_hint: f32,
    estuary_along_blocks: f32,
    config: MacroFieldTileConfig,
) -> f32 {
    let strength = estuary_strength.clamp(0.0, 1.0);
    if strength <= f32::EPSILON || lake_lowering_factor > 0.0 || dry_basin_mask > 0.5 {
        return height;
    }

    let near_sea_t = 1.0 - smoothstep_range(0.015, 0.14, macro_elevation.max(0.0));
    let water_context = ocean_mask
        .clamp(0.0, 1.0)
        .max(coast_mask.clamp(0.0, 1.0) * near_sea_t);
    if water_context <= f32::EPSILON {
        return height;
    }

    let flow_t = smoothstep01(estuary_flow_hint.clamp(0.0, 1.0));
    let bed_t = estuary_bed_depth_hint.clamp(0.0, 1.0);
    let water_active = smoothstep_range(0.72, 1.0, estuary_water_strength.clamp(0.0, 1.0));
    let body_active = smoothstep_range(0.20, 0.90, strength);
    let active = water_active * body_active * water_context;
    let raw_shelf_depth_blocks = (config.river_carve_scale * lerp(0.35, 1.15, flow_t)
        + bed_t * lerp(0.006, 0.026, flow_t))
        * 2048.0
        * ESTUARY_FAN_FINAL_DEPTH_SCALE;
    let terminal_entry_depth_blocks =
        river_core_downcut_budget(estuary_flow_hint, bed_t, None, config) * 2048.0;
    let shallow_start_floor_blocks = lerp(1.5, 3.5, flow_t);
    let final_shelf_depth_blocks = raw_shelf_depth_blocks
        .max(terminal_entry_depth_blocks * ESTUARY_FAN_TERMINAL_FLOOR_GUARD_SCALE)
        .max(shallow_start_floor_blocks);
    let entry_depth_blocks = (terminal_entry_depth_blocks * ESTUARY_FAN_ENTRY_DEPTH_SCALE)
        .max(shallow_start_floor_blocks);
    let grade_budget = estuary_along_blocks.max(0.0) * ESTUARY_FAN_DEPTH_GRADE_BLOCKS_PER_BLOCK;
    let slope_limited_depth_blocks = if final_shelf_depth_blocks >= entry_depth_blocks {
        (entry_depth_blocks + grade_budget).min(final_shelf_depth_blocks)
    } else {
        (entry_depth_blocks - grade_budget).max(final_shelf_depth_blocks)
    }
    .max(0.0);
    let shallow_shelf_depth = slope_limited_depth_blocks / 2048.0;
    let edge_t = smoothstep_range(0.20, 0.92, active);
    let target = -shallow_shelf_depth * lerp(0.16, 1.0, edge_t);
    let carve_strength = smoothstep_range(0.08, 0.96, active);
    let lowering = (height - target).max(0.0) * carve_strength;

    (height - lowering).min(height)
}

#[cfg(test)]
pub(super) fn river_shoulder_context_height(
    macro_elevation: f32,
    river_shoulder_strength: f32,
    river_flow_hint: f32,
    river_centerline_macro_elevation: Option<f32>,
    river_longitudinal_blocks: f32,
    config: MacroFieldTileConfig,
) -> f32 {
    river_shoulder_context_height_with_position(
        macro_elevation,
        river_shoulder_strength,
        river_flow_hint,
        river_centerline_macro_elevation,
        river_longitudinal_blocks,
        None,
        config,
    )
}

fn river_shoulder_context_height_with_position(
    macro_elevation: f32,
    river_shoulder_strength: f32,
    river_flow_hint: f32,
    river_centerline_macro_elevation: Option<f32>,
    river_longitudinal_blocks: f32,
    river_position: Option<WorldPlanePoint>,
    config: MacroFieldTileConfig,
) -> f32 {
    let shoulder = river_shoulder_height_strength(river_shoulder_strength);
    if shoulder <= f32::EPSILON {
        return macro_elevation;
    }

    let flow_t = smoothstep01(river_flow_hint.clamp(0.0, 1.0));
    let log_flow_t = river_shoulder_log_growth(river_flow_hint);
    let context_t = shoulder * lerp(0.18, 0.70, log_flow_t);
    let centerline_t = shoulder * lerp(0.36, 1.00, flow_t);
    let centerline_elevation = river_centerline_macro_elevation
        .filter(|height| height.is_finite())
        .unwrap_or(macro_elevation)
        .min(macro_elevation);
    let centerline_drop = (macro_elevation - centerline_elevation).max(0.0);
    let positive_relief = macro_elevation.max(0.0);
    let terrain_context = smoothstep_range(0.01, 0.18, positive_relief + centerline_drop * 0.8);
    let relief_compression = positive_relief * lerp(0.018, 0.070, log_flow_t);
    let contextual_floor_bias =
        config.river_carve_scale * lerp(0.012, 0.12, log_flow_t) * terrain_context;
    let below_sea_bias = (-macro_elevation).max(0.0) * lerp(0.0, 0.065, log_flow_t);
    let near_sea_t = 1.0 - smoothstep_range(0.0, 0.025, macro_elevation.max(0.0));
    let near_sea_bias =
        config.river_carve_scale * lerp(0.0, 0.075, log_flow_t) * near_sea_t * terrain_context;
    let centerline_pull = centerline_drop * centerline_t * lerp(0.12, 0.42, flow_t);
    let _ = river_longitudinal_blocks;
    let noise_scale = river_context_lowering_noise_scale(river_position, 0.16);
    let broad_lowering =
        (relief_compression + contextual_floor_bias + below_sea_bias + near_sea_bias)
            * context_t
            * noise_scale;
    let max_context_shift = config.river_carve_scale * lerp(0.45, 1.65, log_flow_t)
        + centerline_drop * lerp(0.12, 0.42, flow_t);
    let lowering = (broad_lowering + centerline_pull * noise_scale).min(max_context_shift);

    (macro_elevation - lowering).min(macro_elevation)
}

#[cfg(test)]
pub(super) fn river_core_center_lowering(
    river_core_strength: f32,
    river_flow_hint: f32,
    river_core_depth_hint: f32,
    river_position: Option<WorldPlanePoint>,
    config: MacroFieldTileConfig,
) -> f32 {
    let core_budget = river_core_downcut_budget(
        river_flow_hint,
        river_core_depth_hint,
        river_position,
        config,
    );
    river_core_profile_lowering(river_core_strength, river_flow_hint, core_budget)
}

fn river_core_profile_lowering(
    river_core_strength: f32,
    river_flow_hint: f32,
    core_budget: f32,
) -> f32 {
    if core_budget <= f32::EPSILON {
        return 0.0;
    }
    core_budget * river_core_cross_section_strength(river_core_strength, river_flow_hint)
}

fn river_core_cross_section_strength(river_core_strength: f32, river_flow_hint: f32) -> f32 {
    let core_t = smoothstep_range(
        RIVER_CORE_HEIGHT_PROFILE_THRESHOLD,
        1.0,
        river_core_strength.clamp(0.0, 1.0),
    );
    if core_t <= f32::EPSILON {
        return 0.0;
    }

    let flow_t = smoothstep01(river_flow_hint.clamp(0.0, 1.0));
    let narrow_v = core_t.powf(1.70);
    let broad_u = smoothstep_range(0.16, 0.56, core_t).powf(0.52);

    lerp(narrow_v, broad_u, flow_t).clamp(0.0, 1.0)
}

fn river_bed_bank_lowering(
    river_shoulder_strength: f32,
    river_core_strength: f32,
    river_flow_hint: f32,
    core_budget: f32,
) -> f32 {
    if core_budget <= f32::EPSILON {
        return 0.0;
    }

    let bank_t = river_bed_bank_cross_section_strength(
        river_shoulder_strength,
        river_core_strength,
        river_flow_hint,
    );
    core_budget * 0.50 * bank_t
}

fn river_bed_bank_cross_section_strength(
    river_shoulder_strength: f32,
    river_core_strength: f32,
    river_flow_hint: f32,
) -> f32 {
    let log_flow_t = river_shoulder_log_growth(river_flow_hint);
    let shoulder_cap = lerp(0.86, 0.50, log_flow_t).max(0.05);
    let shoulder_norm = (river_shoulder_strength.clamp(0.0, 1.0) / shoulder_cap).clamp(0.0, 1.0);
    let flow_t = smoothstep01(river_flow_hint.clamp(0.0, 1.0));
    let bank_power = lerp(1.45, 0.78, flow_t);
    let inner_bank = smoothstep_range(0.54, 0.96, shoulder_norm).powf(bank_power);
    let core_presence = smoothstep_range(
        RIVER_CORE_HEIGHT_PROFILE_THRESHOLD * 0.74,
        RIVER_CORE_HEIGHT_PROFILE_THRESHOLD,
        river_core_strength.clamp(0.0, 1.0),
    );

    inner_bank.max(core_presence).clamp(0.0, 1.0)
}

fn river_core_downcut_budget(
    river_flow_hint: f32,
    river_core_depth_hint: f32,
    river_position: Option<WorldPlanePoint>,
    config: MacroFieldTileConfig,
) -> f32 {
    let flow_t = smoothstep01(river_flow_hint.clamp(0.0, 1.0));
    let log_flow_t = river_shoulder_log_growth(river_flow_hint);
    let bed_t = river_core_depth_hint.clamp(0.0, 1.0).max(log_flow_t * 0.34);
    let low_flow_guard = lerp(0.54, 1.0, smoothstep_range(0.06, 0.42, river_flow_hint));
    let base = config.river_carve_scale * lerp(0.04, 1.76, bed_t);
    let flow_floor = config.river_carve_scale * lerp(0.03, 0.48, flow_t);
    let noise_scale = river_context_lowering_noise_scale(river_position, lerp(0.08, 0.21, bed_t));
    let max_shift = config.river_carve_scale * lerp(0.22, 2.55, bed_t);

    ((base + flow_floor) * low_flow_guard * noise_scale).min(max_shift)
}

fn river_morphology_lowering_scale(
    river_position: Option<WorldPlanePoint>,
    river_flow_hint: f32,
    river_bank_roughness_hint: f32,
    river_gravel_hint: f32,
    river_cutbank_hint: f32,
) -> f32 {
    let flow_t = smoothstep01(river_flow_hint.clamp(0.0, 1.0));
    let rough = river_bank_roughness_hint.clamp(0.0, 1.0);
    let gravel = river_gravel_hint.clamp(0.0, 1.0);
    let cutbank = river_cutbank_hint.clamp(0.0, 1.0);
    let random_amp = lerp(0.16, 0.08, flow_t) + rough * 0.08;
    let random_scale = river_position
        .map(|position| {
            let broad =
                smooth_value_noise_2d(position, lerp(96.0, 220.0, flow_t), 0xD1CE_5EED_0101);
            let local = smooth_value_noise_2d(
                WorldPlanePoint::new(position.x - 53.0, position.z + 79.0),
                lerp(28.0, 64.0, flow_t),
                0xD1CE_5EED_0102,
            );
            1.0 + (broad * 0.70 + local * 0.30).clamp(-1.0, 1.0) * random_amp
        })
        .unwrap_or(1.0);
    let bend_scale = 1.0 + cutbank * lerp(0.18, 0.34, flow_t) - gravel * lerp(0.24, 0.38, flow_t);

    (random_scale * bend_scale).clamp(0.52, 1.42)
}

fn river_gravel_bar_fill(
    river_core_strength: f32,
    river_flow_hint: f32,
    river_gravel_hint: f32,
    core_budget: f32,
) -> f32 {
    let gravel = river_gravel_hint.clamp(0.0, 1.0);
    if gravel <= f32::EPSILON || core_budget <= f32::EPSILON {
        return 0.0;
    }
    let core_t = river_core_cross_section_strength(river_core_strength, river_flow_hint);
    let flow_t = smoothstep01(river_flow_hint.clamp(0.0, 1.0));
    core_budget * core_t * gravel * lerp(0.10, 0.22, flow_t)
}

fn river_context_lowering_noise_scale(
    river_position: Option<WorldPlanePoint>,
    amplitude: f32,
) -> f32 {
    let Some(position) = river_position else {
        return 1.0;
    };
    let broad = smooth_value_noise_2d(position, 83.0, 0xD1CE_5EED_0001);
    let medium = smooth_value_noise_2d(
        WorldPlanePoint::new(position.x + 31.0, position.z - 47.0),
        29.0,
        0xD1CE_5EED_0002,
    );
    let noise = (broad * 0.68 + medium * 0.32).clamp(-1.0, 1.0);
    (1.0 + noise * amplitude.clamp(0.0, 0.35)).clamp(0.65, 1.35)
}

pub(super) fn river_shoulder_height_strength(strength: f32) -> f32 {
    let strength = strength.clamp(0.0, 1.0);
    if strength <= f32::EPSILON {
        return 0.0;
    }
    smoothstep01(strength).powf(1.35)
}

pub(super) fn river_shoulder_log_growth(flow_hint: f32) -> f32 {
    ((1.0 + flow_hint.clamp(0.0, 1.0) * 15.0).ln() / 16.0_f32.ln()).clamp(0.0, 1.0)
}

pub(super) fn ocean_bathymetry_macro_height(source_height: f32) -> f32 {
    if source_height >= 0.0 {
        return source_height;
    }

    let depth = (-source_height).max(0.0).clamp(0.0, 1.0);
    if depth <= 0.08 {
        return source_height;
    }

    let shallow_continuity = 0.08;
    let shelf = smoothstep_range(0.08, 0.18, depth) * 0.04;
    let slope = smoothstep_range(0.18, 0.52, depth) * 0.43;
    let basin = smoothstep_range(0.52, 0.92, depth) * 0.45;

    -(shallow_continuity + shelf + slope + basin).clamp(0.0, 1.0)
}

pub(super) fn lake_bed_macro_height(
    source_height: f32,
    lake_lowering_factor: f32,
    config: MacroFieldTileConfig,
) -> f32 {
    let lake_t = lake_lowering_factor.clamp(0.0, 1.0);
    let u_shape = lake_t * lake_t * (3.0 - 2.0 * lake_t);
    let depth = 0.0025 + u_shape * 0.015;
    let target = source_height - depth;
    let preserve_relief = 1.0 - config.lake_flatten_strength.clamp(0.0, 1.0) * 0.42;

    target + (source_height - target) * preserve_relief
}

pub(super) fn roughened_distance(
    distance_blocks: f32,
    position: WorldPlanePoint,
    roughness_blocks: f32,
    salt: u64,
) -> f32 {
    if !distance_blocks.is_finite() || roughness_blocks <= 0.0 {
        return distance_blocks;
    }
    (distance_blocks + boundary_roughness_offset(position, roughness_blocks, salt)).max(0.0)
}

pub(super) fn boundary_roughness_offset(
    position: WorldPlanePoint,
    roughness_blocks: f32,
    salt: u64,
) -> f32 {
    if roughness_blocks <= 0.0 {
        return 0.0;
    }
    let broad = smooth_value_noise_2d(position, 96.0, salt);
    let medium = smooth_value_noise_2d(
        WorldPlanePoint::new(position.x + 37.0, position.z - 61.0),
        41.0,
        salt ^ 0x9E37_79B9_7F4A_7C15,
    );
    let noise = (broad * 0.58 + medium * 0.42).clamp(-1.0, 1.0);
    let shaped = noise.signum() * noise.abs().powf(0.65);
    shaped * roughness_blocks
}

pub(super) fn smooth_value_noise_2d(
    position: WorldPlanePoint,
    scale_blocks: f32,
    salt: u64,
) -> f32 {
    let scale = scale_blocks.max(1.0);
    let x = position.x / scale;
    let z = position.z / scale;
    let x0 = x.floor() as i32;
    let z0 = z.floor() as i32;
    let tx = smootherstep(x - x0 as f32);
    let tz = smootherstep(z - z0 as f32);
    let a = signed_lattice_noise(x0, z0, salt);
    let b = signed_lattice_noise(x0 + 1, z0, salt);
    let c = signed_lattice_noise(x0, z0 + 1, salt);
    let d = signed_lattice_noise(x0 + 1, z0 + 1, salt);
    let top = a + (b - a) * tx;
    let bottom = c + (d - c) * tx;
    top + (bottom - top) * tz
}

pub(super) fn signed_lattice_noise(x: i32, z: i32, salt: u64) -> f32 {
    let mut value = salt;
    value ^= (x as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= (z as i64 as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    let unit = ((value ^ (value >> 31)) as f64 / u64::MAX as f64) as f32;
    unit * 2.0 - 1.0
}

pub(super) fn smootherstep(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}
pub(super) fn is_lake_surface(kind: MacroSurfaceKind) -> bool {
    matches!(
        kind,
        MacroSurfaceKind::LakeCandidate | MacroSurfaceKind::WetlandCandidate
    )
}

pub(super) fn envelope(distance: f32, radius: f32) -> f32 {
    if !distance.is_finite() {
        return 0.0;
    }
    let t = (1.0 - distance / radius.max(f32::EPSILON)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(super) fn smoothstep01(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(super) fn smoothstep_range(edge0: f32, edge1: f32, value: f32) -> f32 {
    let span = (edge1 - edge0).max(f32::EPSILON);
    smoothstep01((value - edge0) / span)
}

pub(super) fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub(super) fn ridge_envelope(distance: f32, radius: f32) -> f32 {
    let raw = envelope(distance, radius);
    if raw <= RIDGE_INFLUENCE_VISIBLE_FLOOR {
        0.0
    } else {
        let t = ((raw - RIDGE_INFLUENCE_VISIBLE_FLOOR) / (1.0 - RIDGE_INFLUENCE_VISIBLE_FLOOR))
            .clamp(0.0, 1.0);
        (t * t * (3.0 - 2.0 * t)).powf(0.92)
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]

    use super::*;
    use crate::world::generation::boundary::{
        BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
    };
    use crate::world::generation::graph::{
        VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId, WorldPlanePoint,
    };
    use crate::world::generation::macro_field::test_support::*;
    use crate::world::generation::macro_field::{
        MacroFieldRasterContext, MacroFieldTileConfig, generate_macro_field_tile,
        sample_macro_field_point,
    };
    use crate::world::generation::macro_map::{
        GraphMacroMap, MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
    };
    use crate::world::generation::river_plan::{
        DEFAULT_RIVER_PLAN_DOWNSTREAM_WATER_WIDTH_BLOCKS, RiverPlan,
    };

    #[test]
    fn ocean_combined_height_preserves_shelf_slope_basin_depth() {
        let config = test_tile_config();
        let shelf = combine_macro_height(-0.08, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, config);
        let slope = combine_macro_height(-0.32, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, config);
        let basin = combine_macro_height(-0.75, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, config);

        assert!(
            shelf > slope && slope > basin,
            "ocean bathymetry should deepen from shelf to slope to basin: shelf={shelf} slope={slope} basin={basin}"
        );
        assert!(
            (shelf - basin).abs() > 0.35,
            "ocean bathymetry should not collapse near sea level: shelf={shelf} basin={basin}"
        );
        assert!(
            slope < -0.12,
            "continental slope should remain visibly below shallow shelf: slope={slope}"
        );
    }

    #[test]
    fn ocean_bathymetry_keeps_coast_adjacent_depth_continuous() {
        let source = -0.004;
        let bathymetry = ocean_bathymetry_macro_height(source);

        assert!(
            (bathymetry - source).abs() < 0.002,
            "coast-adjacent ocean should not jump to a fixed shallow shelf: source={source} bathymetry={bathymetry}"
        );
    }

    #[test]
    fn ocean_bathymetry_preserves_positive_coast_adjacent_source() {
        let source = 0.018;
        let bathymetry = ocean_bathymetry_macro_height(source);

        assert_eq!(
            bathymetry, source,
            "ocean-owned positive source terrain should stay above sea level until heightfield water policy decides coverage"
        );
    }

    #[test]
    fn ocean_bathymetry_preserves_shallow_negative_source_continuity() {
        for source in [-0.012, -0.04, -0.079] {
            let bathymetry = ocean_bathymetry_macro_height(source);

            assert_eq!(
                bathymetry, source,
                "shallow ocean source should continue the signed source field without shelf snapping"
            );
        }
    }

    #[test]
    fn ocean_bathymetry_uses_narrow_shelf_before_slope() {
        let near_coast = ocean_bathymetry_macro_height(-0.03);
        let shelf_edge = ocean_bathymetry_macro_height(-0.08);
        let slope = ocean_bathymetry_macro_height(-0.42);

        assert!(
            shelf_edge < near_coast - 0.025,
            "shelf should narrow quickly after the coast: near={near_coast} shelf_edge={shelf_edge}"
        );
        assert!(
            slope < shelf_edge - 0.2,
            "continental slope should deepen soon after the narrowed shelf: shelf_edge={shelf_edge} slope={slope}"
        );
    }

    #[test]
    fn ocean_bathymetry_no_longer_uses_lake_flatten_strength() {
        let mut weak_flatten = test_tile_config();
        weak_flatten.lake_flatten_strength = 0.0;
        let mut strong_flatten = test_tile_config();
        strong_flatten.lake_flatten_strength = 1.0;

        let weak =
            combine_macro_height(-0.62, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, weak_flatten);
        let strong = combine_macro_height(
            -0.62,
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            strong_flatten,
        );

        assert_eq!(weak, strong);
    }

    #[test]
    fn lake_lowering_still_uses_lake_bed_macro_height() {
        let config = test_tile_config();
        let source = 0.18;
        let factor = 0.72;
        let expected = lake_bed_macro_height(source, factor, config);
        let actual =
            combine_macro_height(source, 0.0, 0.0, 1.0, 0.0, factor, 0.0, 0.0, 0.0, config);

        assert_eq!(actual, expected);
    }

    #[test]
    fn coast_mask_does_not_change_combined_height_but_lake_and_river_still_do() {
        let config = test_tile_config();
        let base = combine_macro_height(0.42, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, config);
        let coast = combine_macro_height(0.42, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, config);
        let lake = combine_macro_height(0.42, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, config);
        let river = combine_macro_height(0.42, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, config);

        assert_eq!(
            base, coast,
            "coast_mask remains diagnostic/downstream policy data and must not flatten combined height"
        );
        assert!(
            lake < base,
            "lake flatten should still lower combined height: lake={lake} base={base}"
        );
        assert!(
            river < base,
            "river broad-valley context should still lower combined height: river={river} base={base}"
        );
        assert_eq!(
            config.river_carve_scale, 0.012,
            "default broad-valley context shift should stay block-scale while core bed depth stays macro-field baked"
        );
    }

    #[test]
    fn river_shoulder_context_reads_centerline_macro_elevation() {
        let config = test_tile_config();
        let without_centerline =
            river_shoulder_context_height(0.48, 0.92, 0.72, None, f32::NAN, config);
        let with_lower_centerline =
            river_shoulder_context_height(0.48, 0.92, 0.72, Some(0.28), f32::NAN, config);
        let with_higher_centerline =
            river_shoulder_context_height(0.48, 0.92, 0.72, Some(0.72), f32::NAN, config);

        assert!(
            with_lower_centerline < without_centerline - config.river_carve_scale,
            "river shoulder should blend toward the projected centerline elevation so contour context follows the river axis: without={without_centerline} with={with_lower_centerline}"
        );
        assert_eq!(
            with_higher_centerline, without_centerline,
            "centerline context may lower banks toward the river floor but must not raise shoulder terrain"
        );
    }

    #[test]
    fn river_shoulder_context_preserves_cross_section_source_relief() {
        let config = test_tile_config();
        let low_source =
            river_shoulder_context_height(0.028, 0.44, 0.82, Some(0.020), f32::NAN, config);
        let high_source =
            river_shoulder_context_height(0.036, 0.44, 0.82, Some(0.020), f32::NAN, config);

        assert!(
            high_source - low_source > 0.004,
            "river shoulder context should lower the valley without flattening cross-section source relief into a contour slab: low={low_source} high={high_source}"
        );
    }

    #[test]
    fn river_shoulder_context_follows_centerline_relief_more_than_uniform_floor() {
        let config = test_tile_config();
        let source = 0.42;
        let flat_context =
            river_shoulder_context_height(source, 0.92, 0.82, Some(source), f32::NAN, config);
        let axis_context =
            river_shoulder_context_height(source, 0.92, 0.82, Some(0.22), f32::NAN, config);
        let low_relief_context =
            river_shoulder_context_height(0.018, 0.92, 0.82, Some(0.018), f32::NAN, config);

        let flat_shift = source - flat_context;
        let axis_shift = source - axis_context;
        let low_relief_shift = 0.018 - low_relief_context;
        assert!(
            axis_shift > flat_shift * 4.0,
            "river shoulder should align banks toward the lower centerline instead of mostly applying a uniform boundary-shaped floor: flat={flat_shift} axis={axis_shift}"
        );
        assert!(
            low_relief_shift < flat_shift * 0.35,
            "uniform lowland/floor bias should be weak when terrain has little source relief to connect to: low_relief={low_relief_shift} flat={flat_shift}"
        );
    }

    #[test]
    fn downstream_river_shoulder_context_is_capped_and_terrain_contextual() {
        let config = test_tile_config();
        let source = 0.42;
        let upstream = river_shoulder_context_height(source, 1.0, 0.05, None, f32::NAN, config);
        let downstream = river_shoulder_context_height(source, 1.0, 1.0, None, f32::NAN, config);
        let near_sea = river_shoulder_context_height(0.012, 1.0, 1.0, None, f32::NAN, config);

        let upstream_shift = source - upstream;
        let downstream_shift = source - downstream;
        let near_sea_shift = 0.012 - near_sea;
        assert!(
            downstream_shift > upstream_shift,
            "downstream shoulder should still grow from upstream context: upstream={upstream_shift} downstream={downstream_shift}"
        );
        assert!(
            downstream_shift > config.river_carve_scale * 1.45
                && downstream_shift <= config.river_carve_scale * 1.75,
            "downstream broad valley context should be visibly carved in macro_field while staying bounded: {downstream_shift}"
        );
        assert!(
            near_sea_shift < downstream_shift * 0.25,
            "fixed floor bias should be terrain-context gated instead of lowering every shoulder sample uniformly: near_sea={near_sea_shift} downstream={downstream_shift}"
        );
    }

    #[test]
    fn river_core_center_profile_deepens_center_more_than_edge() {
        let config = test_tile_config();
        let position = WorldPlanePoint::new(37.0, -91.0);
        let edge = combine_macro_height_with_river_profile(
            0.24,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.89,
            0.65,
            0.60,
            0.0,
            0.0,
            0.0,
            Some(0.18),
            128.0,
            Some(position),
            config,
        );
        let center = combine_macro_height_with_river_profile(
            0.24,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            1.0,
            0.65,
            0.60,
            0.0,
            0.0,
            0.0,
            Some(0.18),
            128.0,
            Some(position),
            config,
        );

        assert!(
            center < edge - config.river_carve_scale * 0.55,
            "river core center should be cut noticeably below the near-edge core in macro_field: edge={edge} center={center}"
        );
    }

    #[test]
    fn river_bed_bank_profile_lowers_near_bank_by_half_core_budget() {
        let config = test_tile_config();
        let core_budget = river_core_downcut_budget(0.62, 0.55, None, config);
        let near_bank = river_bed_bank_lowering(0.60, 0.0, 0.62, core_budget);
        let outer_shoulder = river_bed_bank_lowering(0.18, 0.0, 0.62, core_budget);
        let core_edge = river_bed_bank_lowering(0.60, 0.89, 0.62, core_budget);

        assert!(
            near_bank >= core_budget * 0.42 && near_bank <= core_budget * 0.50,
            "near river bank lowering should be roughly half the core depth budget: budget={core_budget} bank={near_bank}"
        );
        assert_eq!(
            core_edge,
            core_budget * 0.50,
            "the immediate riverbed/core edge should share the same half-depth bank baseline"
        );
        assert!(
            outer_shoulder < near_bank * 0.15,
            "broad valley shoulder should not receive the bed/bank half-depth cut: outer={outer_shoulder} bank={near_bank}"
        );
    }

    #[test]
    fn low_flow_core_is_narrow_v_and_depth_guarded() {
        let config = test_tile_config();
        let budget = river_core_downcut_budget(0.04, 3.0 / 40.0, None, config);
        let center = river_core_profile_lowering(1.0, 0.04, budget);
        let off_center = river_core_profile_lowering(0.92, 0.04, budget);

        assert!(
            center < config.river_carve_scale * 0.14,
            "low-Q headwater core should not downcut so deeply that the cross-section reads like a fault: center={center}"
        );
        assert!(
            off_center < center * 0.22,
            "low-Q river core should stay narrow and V-shaped across the section: center={center} off_center={off_center}"
        );
    }

    #[test]
    fn high_flow_core_blends_toward_broad_u_section() {
        let low_q_mid = river_core_cross_section_strength(0.94, 0.04);
        let high_q_mid = river_core_cross_section_strength(0.94, 0.92);
        let high_q_center = river_core_cross_section_strength(1.0, 0.92);

        assert!(
            high_q_mid > high_q_center * 0.75,
            "high-Q riverbed should keep a wide U-shaped bottom instead of a narrow point: mid={high_q_mid} center={high_q_center}"
        );
        assert!(
            high_q_mid > low_q_mid * 2.0,
            "the same cross-section position should be much broader at high Q than at low Q: low={low_q_mid} high={high_q_mid}"
        );
    }

    #[test]
    fn river_morphology_hints_reduce_inside_bar_and_deepen_cutbank() {
        let config = test_tile_config();
        let position = WorldPlanePoint::new(23.0, -41.0);
        let base = combine_macro_height_with_river_profile(
            0.24,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.94,
            0.97,
            0.72,
            0.58,
            0.35,
            0.0,
            0.0,
            Some(0.16),
            128.0,
            Some(position),
            config,
        );
        let gravel_bar = combine_macro_height_with_river_profile(
            0.24,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.94,
            0.97,
            0.72,
            0.58,
            0.35,
            0.90,
            0.0,
            Some(0.16),
            128.0,
            Some(position),
            config,
        );
        let cutbank = combine_macro_height_with_river_profile(
            0.24,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.94,
            0.97,
            0.72,
            0.58,
            0.35,
            0.0,
            0.90,
            Some(0.16),
            128.0,
            Some(position),
            config,
        );

        assert!(
            gravel_bar > base,
            "gravel-bar hints should weaken local riverbed downcut: base={base} gravel={gravel_bar}"
        );
        assert!(
            cutbank < base,
            "cutbank hints should strengthen local riverbed downcut: base={base} cutbank={cutbank}"
        );
    }

    #[test]
    fn river_core_downcut_has_deterministic_world_space_variation() {
        let config = test_tile_config();
        let first = river_core_center_lowering(
            1.0,
            0.72,
            0.58,
            Some(WorldPlanePoint::new(13.0, 29.0)),
            config,
        );
        let second = river_core_center_lowering(
            1.0,
            0.72,
            0.58,
            Some(WorldPlanePoint::new(47.0, 29.0)),
            config,
        );
        let repeat = river_core_center_lowering(
            1.0,
            0.72,
            0.58,
            Some(WorldPlanePoint::new(13.0, 29.0)),
            config,
        );

        assert_eq!(first, repeat);
        assert_ne!(
            first, second,
            "macro river downcut should include deterministic non-uniformity instead of a perfectly artificial trough"
        );
    }

    #[test]
    fn estuary_fan_lowers_only_coast_or_ocean_near_sea_source() {
        let config = test_tile_config();
        let coast = estuary_fan_macro_height(
            0.035, 0.035, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.85, 0.70, 96.0, config,
        );
        let ocean = estuary_fan_macro_height(
            0.020, 0.020, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.85, 0.70, 96.0, config,
        );
        let ordinary = estuary_fan_macro_height(
            0.035, 0.035, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.85, 0.70, 96.0, config,
        );

        assert!(
            coast <= 0.0,
            "estuary fan should open near-sea coast source to sea level or below: {coast}"
        );
        assert!(
            ocean <= 0.0,
            "estuary fan should keep ocean-side mouth bed connected below sea level: {ocean}"
        );
        assert_eq!(
            ordinary, 0.035,
            "ordinary land/no-flow samples must not be carved by estuary fan strength alone"
        );
    }

    #[test]
    fn estuary_fan_edge_strength_does_not_snap_water_boundary_below_sea() {
        let config = test_tile_config();
        let weak_edge = estuary_fan_macro_height(
            0.020, 0.020, 0.0, 1.0, 0.0, 0.0, 0.42, 0.04, 0.85, 0.70, 96.0, config,
        );
        let soft_edge = estuary_fan_macro_height(
            0.020, 0.020, 0.0, 1.0, 0.0, 0.0, 0.70, 0.62, 0.85, 0.70, 96.0, config,
        );
        let core = estuary_fan_macro_height(
            0.020, 0.020, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.85, 0.70, 96.0, config,
        );

        assert!(
            weak_edge > 0.0,
            "weak estuary fan edge samples should taper toward the bank instead of creating one-block water speckles: {weak_edge}"
        );
        assert!(
            (0.020 - weak_edge) * 2048.0 <= 3.0,
            "weak estuary fan edge samples should not carve a visible terminal seam: {weak_edge}"
        );
        assert!(
            (0.020 - soft_edge) * 2048.0 <= 3.0,
            "mid-strength estuary fan edges should stay shallow instead of creating a ring seam: {soft_edge}"
        );
        assert!(
            core <= 0.0,
            "full estuary fan core should still open the near-sea mouth to water: {core}"
        );
    }

    #[test]
    fn estuary_fan_start_depth_matches_terminal_river_context() {
        let config = test_tile_config();
        let flow_hint = 0.90;
        let bed_depth_hint = 0.70;
        let terminal_depth_blocks =
            river_core_downcut_budget(flow_hint, bed_depth_hint, None, config) * 2048.0;
        let expected_entry_depth_blocks = (terminal_depth_blocks * ESTUARY_FAN_ENTRY_DEPTH_SCALE)
            .max(lerp(1.5, 3.5, smoothstep01(flow_hint)));
        let mut heights = Vec::new();
        let mut max_delta_blocks = 0.0_f32;
        let mut previous_height = None::<f32>;

        for along in 0..=24 {
            let height = estuary_fan_macro_height(
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
                0.0,
                1.0,
                1.0,
                flow_hint,
                bed_depth_hint,
                along as f32,
                config,
            ) * 2048.0;
            if let Some(previous) = previous_height {
                max_delta_blocks = max_delta_blocks.max((height - previous).abs());
            }
            previous_height = Some(height);
            heights.push(height);
        }

        eprintln!("estuary start heights blocks={heights:?} max_delta={max_delta_blocks:.3}");
        assert!(
            max_delta_blocks <= 0.58,
            "estuary fan start should not fall faster than a 30 degree grade: max_delta={max_delta_blocks}"
        );
        assert!(
            (heights[0] + expected_entry_depth_blocks).abs() <= 0.001,
            "estuary fan origin should use a raised entry shelf instead of copying a round terminal bowl: heights={heights:?} entry_depth={expected_entry_depth_blocks}"
        );
    }

    #[test]
    fn estuary_fan_final_depth_uses_raised_sea_floor_guard() {
        let config = test_tile_config();
        let flow_hint = 0.90;
        let bed_depth_hint = 0.70;
        let flow_t = smoothstep01(flow_hint);
        let unscaled_shelf_depth_blocks = (config.river_carve_scale * lerp(0.35, 1.15, flow_t)
            + bed_depth_hint * lerp(0.006, 0.026, flow_t))
            * 2048.0;
        let terminal_depth_blocks =
            river_core_downcut_budget(flow_hint, bed_depth_hint, None, config) * 2048.0;
        let final_height_blocks = estuary_fan_macro_height(
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            1.0,
            1.0,
            flow_hint,
            bed_depth_hint,
            4096.0,
            config,
        ) * 2048.0;
        let expected_depth_blocks = (unscaled_shelf_depth_blocks * ESTUARY_FAN_FINAL_DEPTH_SCALE)
            .max(terminal_depth_blocks * ESTUARY_FAN_TERMINAL_FLOOR_GUARD_SCALE)
            .max(lerp(1.5, 3.5, flow_t));
        let expected_height_blocks = -expected_depth_blocks;

        assert!(
            (final_height_blocks - expected_height_blocks).abs() <= 0.001,
            "estuary fan final reach should use the raised sea-floor target instead of being pinned to the terminal river floor: final={final_height_blocks}, expected={expected_height_blocks}"
        );
    }

    #[test]
    fn estuary_fan_excludes_lake_and_dry_basin_sources() {
        let config = test_tile_config();
        let lake = estuary_fan_macro_height(
            0.020, 0.020, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0, 0.85, 0.70, 96.0, config,
        );
        let dry = estuary_fan_macro_height(
            0.020, 0.020, 0.0, 1.0, 0.0, 1.0, 1.0, 1.0, 0.85, 0.70, 96.0, config,
        );

        assert_eq!(lake, 0.020);
        assert_eq!(dry, 0.020);
    }

    #[test]
    fn weak_river_shoulder_tail_is_continuous_but_attenuated_for_height() {
        let config = test_tile_config();
        let base = river_shoulder_context_height(0.34, 0.0, 0.8, Some(0.12), 384.0, config);
        let weak = river_shoulder_context_height(0.34, 0.18, 0.8, Some(0.12), 384.0, config);
        let active = river_shoulder_context_height(0.34, 0.74, 0.8, Some(0.12), 384.0, config);

        let weak_shift = base - weak;
        let active_shift = base - active;
        assert!(
            weak_shift > 0.0,
            "weak shoulder tails should remain continuous"
        );
        assert!(
            weak_shift < active_shift * 0.20,
            "weak broad-shoulder tails should be strongly attenuated without creating a hard contour cutoff"
        );
        assert!(
            active < base,
            "active river shoulder should still lower combined height near the selected river corridor"
        );
    }

    #[test]
    fn river_shoulder_height_does_not_step_on_longitudinal_hint_switch() {
        let config = test_tile_config();
        let left_hint =
            river_shoulder_context_height(0.036, 0.90, 0.75, Some(0.024), 116.0, config);
        let right_hint =
            river_shoulder_context_height(0.036, 0.90, 0.75, Some(0.024), 172.0, config);

        assert_eq!(
            left_hint, right_hint,
            "longitudinal hints are diagnostics/downstream hints; combined macro height must not form a vertical seam when nearest river segment ownership switches"
        );
    }

    #[test]
    fn ocean_owned_broad_river_valley_lowers_without_forcing_positive_source_below_sea_level() {
        let config = test_tile_config();
        let source = 0.01;
        let carved = combine_macro_height(0.01, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, config);

        assert!(
            carved < source && carved > 0.0,
            "selected river broad-valley context should lower ocean-owned positive source without snapping it below sea level: {carved}"
        );
    }

    #[test]
    fn lake_lowering_carves_a_rounded_bed_without_flattening_to_one_height() {
        let config = test_tile_config();
        let source = 0.18;
        let shore = combine_macro_height(source, 0.0, 0.0, 1.0, 0.0, 0.25, 0.0, 0.0, 0.0, config);
        let slope = combine_macro_height(source, 0.0, 0.0, 1.0, 0.0, 0.65, 0.0, 0.0, 0.0, config);
        let center = combine_macro_height(source, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, config);
        let higher_source =
            combine_macro_height(0.24, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, config);

        assert!(source > shore && shore > slope && slope > center);
        assert!(
            higher_source > center,
            "lake bed should preserve source relief instead of collapsing all lake interiors to a flat target"
        );
    }

    #[test]
    fn dry_basin_mask_does_not_add_macro_field_lowering() {
        let config = test_tile_config();
        let base = combine_macro_height(0.18, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, config);
        let dry_height = combine_macro_height(0.18, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, config);
        let water_height =
            combine_macro_height(0.18, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, config);

        assert_eq!(
            dry_height, base,
            "dry basin mask should preserve macro elevation instead of adding a floor/lowering profile"
        );
        assert!(
            dry_height > water_height,
            "dry basin should not use lake/ocean water flatten: dry={dry_height} water={water_height}"
        );
    }

    #[test]
    fn lake_lowering_factor_transitions_across_boundary() {
        let lake = test_site(
            crate::world::generation::graph::VoronoiSiteId(1),
            -10.0,
            0.0,
            MacroSurfaceKind::LakeCandidate,
            -0.04,
        );
        let land = test_site(
            crate::world::generation::graph::VoronoiSiteId(2),
            10.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.24,
        );
        let radius = 32.0;
        let lake_edge = lake_boundary_lowering_factor(lake, 0.0, radius, 0.0);
        let land_edge = lake_boundary_lowering_factor(land, 0.0, radius, 0.0);
        let lake_interior = lake_boundary_lowering_factor(lake, radius, radius, 0.0);
        let land_exterior = lake_boundary_lowering_factor(land, radius, radius, 0.0);

        assert_eq!(lake_edge, 0.0);
        assert_eq!(land_edge, 0.0);
        assert!(lake_interior > lake_edge);
        assert_eq!(lake_interior, 1.0);
        assert_eq!(land_exterior, 0.0);
    }

    #[test]
    fn boundary_roughness_perturbs_visible_distance_without_changing_owner_masks() {
        let position = WorldPlanePoint::new(37.0, -91.0);
        let smooth = roughened_distance(48.0, position, 0.0, 17);
        let rough = roughened_distance(48.0, position, 96.0, 17);

        assert_eq!(smooth, 48.0);
        assert_ne!(rough, smooth);
        assert!(
            (rough - smooth).abs() <= 96.0,
            "boundary roughness should stay bounded: smooth={smooth} rough={rough}"
        );
    }

    #[test]
    fn default_combined_height_does_not_apply_ridge_raise() {
        let config = MacroFieldTileConfig::new(0.0, 0.0, 1, 1, 32.0);
        let without_ridge =
            combine_macro_height(0.20, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, config);
        let with_ridge_influence =
            combine_macro_height(0.20, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, config);

        assert_eq!(
            config.ridge_height_scale, 0.0,
            "launch macro field keeps ridge influence diagnostic-only until broad mountain elevation is reintroduced"
        );
        assert_eq!(
            without_ridge, with_ridge_influence,
            "ridge influence should not create pinpoint combined-height maxima while ridge raise is disabled"
        );
    }

    #[test]
    fn combined_macro_height_is_lower_near_river_curve_than_far_terrain() {
        let inputs = test_inputs(42);
        let Some(segment) = inputs.hydrology.segments.first() else {
            return;
        };
        let curve = inputs
            .boundary
            .curve_for_edge(segment.edge)
            .expect("selected river edge should have canonical boundary curve");
        let context = MacroFieldRasterContext::new(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.river_plan,
            &inputs.boundary,
        );
        let config = test_tile_config();
        let near = curve.points[curve.points.len() / 2];
        let far = WorldPlanePoint::new(
            near.x + config.river_radius_blocks * 2.4,
            near.z + config.river_radius_blocks * 2.4,
        );
        let near_sample = sample_macro_field_point(&context, config, near);
        let far_sample = sample_macro_field_point(&context, config, far);

        assert!(
            near_sample.combined_macro_height < far_sample.combined_macro_height,
            "river broad-valley lowering should be visible in combined macro height: near={} far={}",
            near_sample.combined_macro_height,
            far_sample.combined_macro_height
        );
    }
}
